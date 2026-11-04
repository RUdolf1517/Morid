# Аудит оригинальной кодовой базы (C++ / Qt)

Аудит форка **Media Downloader** (upstream: `mhogomchungu/media-downloader`), версия в дереве —
**5.6.8**, дата последней записи в `changelog` — 4 ноября 2026.

Документ описывает **что есть сейчас**, а не что планируется. Все утверждения опираются на файлы
репозитория; ссылки даны в виде `файл:строка`. Там, где вывод сделан по чтению кода без запуска
(сборка не выполнялась: в окружении аудита нет Qt и нет Rust-тулчейна), это сказано прямо.

Статус: рабочий документ менеджера проекта, подлежит уточнению по мере чтения кода.

---

## 1. Краткое описание текущего состояния

| Параметр | Значение |
| --- | --- |
| Язык | C++14, Qt 5.15 или Qt 6 (`CMakeLists.txt:11,32-52`) |
| Система сборки | CMake + qmake-проект (`src/media-downloader.pro`) как второй путь |
| Объём кода | 42 910 строк: 37 `.cpp`, 35 `.h`, 8 `.hpp` |
| Файлы всего / размер дерева | 202 файла / ~12 MB |
| UI | 2 `.ui`-файла, 2 958 строк (`mainwindow.ui` 2 872, `logwindow.ui` 86) |
| Локализация | 18 каталогов `.ts`/`.qm`, по 354 сообщения в каждом |
| Расширения | 17 JSON-описаний движков в `extensions/` |
| Лицензия | GPL-2.0-or-later (`LICENSE` = текст GPLv2, `LICENSE.txt` = уведомление об авторстве) |
| CI | 10 workflow, все — сборка/публикация артефактов. **Тестов нет ни в одном** |
| Контроль версий | один коммит истории в этом форке (`46e881e code improvement`) |
| История | 71 релиз с 1.0.0 (январь 2021) по 5.6.8 |

Репозиторий — это форк, поэтому основная ценность для нас: рабочее поведение, JSON-схема расширений и
набор файлов данных пользователя. Git-история upstream в форк не перенесена, что усложняет
атрибуцию авторства при переносе кода; уведомления об авторстве в файлах сохранены.

Аудит выявил три обстоятельства, которые влияют на план больше, чем объём кода:

1. **В оригинале нет автотестов.** Единственная проверка — скрытый CLI-режим прогона движков
   (`src/engines/tests.cpp`, флаги `--media-downloader-test-engine-*`). То есть «эталон поведения»
   придётся фиксировать нам, своими руками, иначе сравнивать Rust-версию будет не с чем.
2. **Ядро приложения — не GUI, а разбор вывода сторонних CLI.** Большая часть сложности
   (3 024 строки `engines.cpp`, 2 083 строки `engines.h`, 1 853 строки `engines/yt-dlp.cpp`,
   986 строк `logger.h`) — это парсинг прогресса и метаданных из stdout yt-dlp, gallery-dl, wget,
   aria2c, svtplay-dl, get-sauce. Формат этого вывода контролируется не нами.
3. **UI-слой тесно переплетён с логикой.** Обработчики `tabManager`/`batchdownloader`/
   `playlistdownloader` вызывают движки напрямую; отдельного «сервисного» слоя нет. При переписывании
   на Rust это главный источник архитектурной работы, а не GUI-виджеты.

---

## 2. Анализ архитектуры оригинала

### 2.1 Точка входа и жизненный цикл

`src/main.cpp`:

- разбор `argv` в `utility::cliArguments` (`src/utility.cpp:1705`);
- определение платформы, включая «притворные» платформы для отладки
  (`PretendPlatform`, флаги `--pretend-win`, `--pretend-winLegacy`, `--pretend-winModern`);
- `settings` + `engines::enginePaths`;
- ветка самообновления `utility::startedUpdatedVersion` — **только Windows**
  (`src/utility.cpp:1358`): переименование каталогов `update`/`update_new` при старте;
- скрытый режим тестов движков (`tests::test_engine`);
- выбор однопользовательского или многопользовательского режима по `-s` или настройке `SingleInstance`
  (`src/main.cpp:93`), реализация — `QLocalServer`/`QLocalSocket`
  (`src/utils/single_instance.hpp`).

Межпроцессный протокол: JSON-событие (`utility::event::toJson`, `src/utility.h:699`), поля
`Url`, `AutoDownload`, `ShowMetaData`, `Proxy`; путь сокета —
`<basePath>/tmp/ipc` на Unix, `\\.\pipe\MediaDownloaderIPC` на Windows
(`src/engines.cpp:1350-1360`).

### 2.2 Композиция: `Context` как локатор сервисов

`src/context.hpp` — `Context` собирает ссылки на `settings`, `translator`, `Ui::MainWindow`, `QWidget`,
`MainWindow`, `Logger`, `engines`, `tabManager`, `networkAccess`, `versionInfo`, `printOutPut`.

Последствия для оценки:

- **плюс**: всё явно передаётся, глобальных синглтонов почти нет;
- **минус**: `Context` — это 15 зависимостей в одном объекте; любой модуль зависит от всего.
  Любой тест любого модуля требует поднять `QApplication`, UI и логгер. Отсюда — отсутствие
  юнит-тестов: архитектура их не допускает;
- **минус**: циклические зависимости: `Context` → `tabManager` → `basicdownloader` → `Context`.

### 2.3 Слой движков — «god object»

`src/engines.h` (2 083 строки) содержит в одном файле:

- `engines::enginePaths` — все пути данных (`src/engines.h:202-330`);
- `engines::metadata` — метаданные релиза с GitHub;
- `engines::engine` — обёртка-фасад;
- `engines::engine::baseEngine` — базовый класс движка с **57 виртуальными методами**;
- `engines::engine::baseEngine::filter / filterOutPut / DataFilter / FilterOutPut / timer /
  preProcessing / postProcessing / optionsEnvironment / mediaInfo / finishedState`;
- `engines::engine::jsRuntimeInstalled`, `engines::EnginesList`, `engines::Iterator`,
  `engines::proxySettings`.

Глубина вложенности типов доходит до
`engines::engine::baseEngine::filterOutPut::result::callables` (`src/engines.h:660`).

57 виртуальных методов — это не «интерфейс движка», это «всё, что движок может захотеть
переопределить»: разбор версии, разбор JSON, правка аргументов командной строки, конвертация
аргументов в переменные окружения, удаление каталога, переименование архива, текст по завершении,
условия пропуска строки, политика обновления. Фактически — наследование вместо композиции.

### 2.4 Наследники `baseEngine` (13 файлов в `src/engines/`)

| Файл | Что специфично |
| --- | --- |
| `yt-dlp.cpp` (1 853 стр.) | разбор JSON лота/metadata, шаблоны `-o` с подстановкой плейлист-полей (`updateDownLoadCmdOptions`, ~стр. 1493), субтитры, комментарии, главы |
| `gallery-dl.cpp` (736) | поток JSON-строк, свой `parseJsonData`/`parseJson` |
| `svtplay-dl.cpp` (1 041) | свой фильтр вывода и `updateVersionString` |
| `getsauce.cpp` (1 046) | баннеры, свой прогресс, конвертация аргументов в env |
| `wget.cpp` (765), `aria2c.cpp` (325) | прогресс-строки, `trimProgressLine`, `bundledEngine` |
| `you-get.cpp` (279), `python.cpp` | обёртки над интерпретатором Python |
| `ffmpeg.cpp` (226), `deno.cpp` (183), `quickjs_ng.cpp` (193) | вспомогательные («supporting engine») бинарники |
| `generic.cpp` | пустая реализация — всё из JSON |
| `tests.cpp` (185) | скрытый режим проверки движков |

Вывод: **JSON-схема расширения покрывает не всё**. Часть движков требует кода на стороне приложения.
Это ключевое ограничение для идеи «Rust-версия читает те же JSON и работает».

### 2.5 Схема расширения (фактический контракт)

Собрано по всем 17 файлам `extensions/*.json`: 34 различных ключа.

```
ArchiveContainsFolder, AutoUpdate, BackendPath, BatchFileArgument, CanDownloadPlaylist,
Cmd, ControlJsonStructure, CookieArgument, CookieArgumentTextFile, DefaultCommentsCmdOptions,
DefaultDownLoadCmdOptions, DefaultListCmdOptions, DefaultSubstitlesCmdOptions,
DefaultSubtitleDownloadOptions, DownloadUrl, DownloadUrlMAC, DownloadUrlWin7,
DumptJsonArguments, EncodingArgument, ExtraOptions, ExtraOptionsFlatpakAppImage,
ExtraOptionsWin7, LikeYoutubeDl, Name, OptionsArgument, Password, PlaylistItemsArgument,
RemoveText, ReplaceOutputWithProgressReport, RequiredMinimumVersionOfMediaDownloader,
SkipLineWithText, SplitLinesBy, SupportingEngine, UpdatableSupportingEngine,
UserName, Version, VersionArgument, VersionStringLine, VersionStringPosition
```

`Cmd` вложен по ОС (`Generic` / `Windows` / `MacOS`) и архитектуре
(`amd64`, `x86`, `aarch64`, `win7amd64`, `win7x86`) — встречаются 34 комбинации.

`ControlJsonStructure` — мини-язык условий на строку: `Connector` (`&&`), `lhs`/`rhs` с
операторами `startsWith`, `endsWith`, `contains`, `containsAny`, `containsAll`
(реализация — `meetExtraCondition`, `src/engines.cpp:1494-1580`). Это способ отделить строки
прогресса от прочего вывода без регулярных выражений.

Дефекты и несогласованности схемы (найдены при аудите):

1. **`ExtraOptions*` vs `ExtraArguments*`.** Код читает `ExtraOptions`,
   `ExtraOptionsWin7`, `ExtraOptionsFlatpakAppImage` (`src/engines.cpp:1052-1058`), а в
   `extensions/ytdl-patched.json` лежат `ExtraArguments`, `ExtraArgumentsFlatpak`,
   `ExtraArgumentsWin7` — **код их не читает**. То есть в поставляемом наборе есть расширение,
   три ключа которого молча игнорируются. Это область неопределённости контракта, которую нужно
   либо закрепить, либо исправить.
2. `DefaultSubstitlesCmdOptions` — опечатка в имени ключа зафиксирована в 5 файлах и должна
   сохраняться для совместимости.
3. `RequiredMinimumVersionOfMediaDownloader` проверяется при загрузке (`src/engines.cpp:264-272`):
   расширение с завышенным минимумом отвергается. При смене версии продукта на Morid это ломает
   совместимость со всеми существующими расширениями — см. риски.
4. `Version` в JSON — версия схемы расширения (1..4), но семантика различий не документирована;
   код читает поле и почти не ветвится по нему.
5. Откуда берутся расширения: жёстко зашитый коммит upstream
   (`https://raw.githubusercontent.com/mhogomchungu/media-downloader/d20c57211494276b206b19c021d358746de5ba29/extensions/`,
   `src/configure.cpp:876`) и список из 10 имён + `custom` в меню (`src/configure.cpp:1231-1252`),
   при том что в репозитории лежат 17 файлов (`lux`, `safaribooks`, `yt-dlp-aria2c-native`,
   `quickjs*`, `deno`, `ytdl-patched` в меню не выводятся).

### 2.6 Конвейер загрузки

`utility::context<AppCtx, Logger, Events, Connection>` (`src/utility.h:1873-2135`) — шаблонная обёртка
над `QProcess`:

- `whenCreated` — окружение, рабочая папка, режим каналов, печать команды в лог;
- `whenStarted` — периодический таймер «прошло времени» для движков с
  `ReplaceOutputWithProgressReport`, отправка credentials в stdin;
- `withData` — маршрутизация stdout/stderr (`ProcessOutputChannels`) и передача в `Logger`;
- `whenDone` — `ProcessExitState`, финальный текст, `registerDone`;
- `cmd(args)` — навешивание прокси (`--proxy` как аргумент **или** как переменные окружения) и
  конвертация аргументов в env через движок.

Запуск и отмена: `utils::qprocess::run` (`src/utils/qprocess.hpp`), `utility::Terminator`
(`src/utility.cpp:621-680`) — на Windows `taskkill`, на Unix `SIGTERM` по PID.

Оркестрация вкладок: `basicdownloader` (694 стр.), `batchdownloader` (3 208 стр.),
`playlistdownloader` (1 863 стр.) — каждая вкладка сама управляет своими процессами, очередью,
состоянием строк таблицы и отчётами о завершении. Единого планировщика нет: «очередь» — это
последовательный обход строк таблицы с флагами `running/finished/splitByChapters/...`
(`src/tableWidget.h:94-180`).

Ограничение параллельности — `MaxConcurrentDownloads` (по умолчанию 4,
`src/settings.cpp:623`) и `ConcurrentDownloading`.

### 2.7 `Logger` — не логирование, а модель состояния прогресса

`src/logger.h` (986 строк): кольцевые буферы вывода на процесс, «прогресс-строки» (замена последней
строки вместо накопления), специализации `ProcessData` / `YtDlpData` / `SvtData`, лимиты
(`MaxLoggerProcesses`, `HistorySize`, `StringTruncationSize`), отдельное окно лога, журнал истории
загрузок (`downloadHistory.json`, дописывание JSON-объектов и поиск подстрокой —
`src/utility.cpp:2470-2520`).

Фактически это и «лог», и «парсер прогресса», и «хранилище результатов». При переносе разделяется на
три части.

### 2.8 Сеть и обновления

`src/networkAccess.cpp` (1 168 строк):

- загрузка бинарников движков с GitHub (`api.github.com/.../releases/latest`), проверка размера и
  SHA-256 (`digest`) — `hashDoNotMatch` (`src/networkAccess.cpp:918`);
- распаковка архивов через внешние `bsdtar`/`unzip` (`src/networkAccess.cpp:929-990`);
- обновление самих расширений и самообновление приложения (Windows,
  `settings::gitHubDownloadUrl`, `src/settings.cpp:353-372`);
- прокси: `None` / `System` / `Env` / `Manual` (`src/settings.cpp:1416-1470`), определение системного
  прокси из переменных (`src/proxy.cpp`).

Для форка Morid все эти адреса указывают на `mhogomchungu` и на зашитый коммит — это придётся менять
целиком (см. §6, риск R7).

### 2.9 GUI

`src/mainwindow.ui` (2 872 строки), вкладки:

| Вкладка | Содержимое |
| --- | --- |
| Basic Downloader | URL, список форматов (Format Code / Extension / Resolution / File Size / Note), опции, история опций, «Play Downloaded Media» |
| Batch Downloader | таблица URL с превью, интервалы времени, главы, «Split By Chapters», добавление из файла/буфера, автозагрузка |
| Playlist Downloader | URL плейлиста, диапазон, подписки (subscriptions), архив, комментарии, субтитры |
| Library | обзор папки загрузок, переименование, удаление, сортировки |
| Configure | 6 подвкладок: General Options, Engine's Options, Preset Options, Extensions, Url Manager, Proxy Settings, UI Scale |
| About | ссылки, версии движков |

Плюс `logwindow.ui`, трей-иконка с уведомлениями (`src/mainwindow.cpp:88-218`), тёмная тема
(генерируемые JSON-темы в `<data>/themes`, `src/themes.cpp:201-280`), RTL-выравнивание при арабском
(`translator::setLanguage` → `setLayoutDirection` + `textAlignmentChanged`).

### 2.10 Настройки и файлы данных

`QSettings`, INI, `<AppData>/media-downloader/settings/settings.ini`
(`src/settings.cpp:374-410`); каталог переопределяется переменной `MediaDownloaderAppDataPath`
(`src/settings.cpp:423`); портативный режим на Windows. 51 уникальный ключ через `getOption`,
61 с учётом динамических (`CookieBrowser_<engine>`, `LastUsedOption<tab>_<engine>`,
`TextEncoding_<engine>`, `WindowDimensions_<tab>`). Есть миграция со старого реестрового
`QSettings("media-downloader","media-downloader")`.

Файлы данных в `<data>/`:

| Файл | Назначение |
| --- | --- |
| `presetOptions.json` | 17 предустановленных наборов опций (разрешения YouTube) |
| `downloadDefaultOptions.json` | Url Manager: подстрока URL → движок + опции |
| `downloadEngineDefaultOptions.json` | опции по умолчанию для каждого движка |
| `subscriptions.json` | подписки на плейлисты |
| `autoSavedList.json` | автосохранённый список batch/playlist |
| `downloadHistory.json` | история загрузок |
| `archiveFile-<engine>.txt` / `.db` | архив уже скачанного (`--download-archive`) |
| `themes/*.json` | темы |

### 2.11 Локализация

18 каталогов: `ar, bg_BG, de_DE, el_GR, en_US, es_ES, fr_FR, it_IT, ja_JP, ko_KR, nl_NL, pl_PL,
pt_BR, ru_RU, sv_SE, tr_TR, uk_UA, zh_CN`. По 354 сообщения. Строки в коде через `tr()`, в UI — в
`.ui`. Названия языков дополнительно переводятся (`src/translator.cpp:32-52`). Формат — Qt `.ts`/`.qm`.
Документации по переводу в репозитории нет; есть скрипт `src/update_translations`.

### 2.12 Сборка и распространение

- CMake: Qt5/Qt6, `AUTOMOC/AUTORCC/AUTOUIC`, C++14; генерация `version.h`, `.desktop`, AppStream
  metainfo, macOS bundle, Inno Setup скриптов для Qt5 и Qt6 внутри `CMakeLists.txt`.
- Скрипты: `build_linux-qt5.sh`, `build_arch-qt5.sh`, `build_fedora-qt5.sh`, `build_windows.ps1`.
- Flatpak-манифест (`src/flatpak/*.json`), AUR, Fedora, AppImage — снаружи.
- CI (10 workflow): сборки macOS arm/Intel, Windows (Qt5), публикация через `repository-dispatch` в
  `mhogomchungu/media-downloader-git` по секретам. **В форке эти workflow неработоспособны** (нет
  секретов, нет целевых репозиториев) и, главное, ни один не запускает тесты.

### 2.13 Технические ограничения и дефекты, зафиксированные аудитом

| # | Наблюдение | Где |
| --- | --- | --- |
| D1 | `metadata::valid()` проверяет `m_url` дважды и не проверяет hash/имя файла — релиз без digest считается валидным | `src/engines.h:344-350` |
| D2 | Ошибки передаются строками и распознаются поиском подстроки: `m.contains("Download Failed(ErrorCode=1)")` | `src/playlistdownloader.cpp:1791-1795` |
| D3 | `baseEngine` — 57 виртуальных методов, глубоко вложенные типы | `src/engines.h` |
| D4 | Нет юнит-тестов; проверка движков — скрытый CLI-режим с сетью | `src/engines/tests.cpp` |
| D5 | Расширения скачиваются по зашитому коммиту upstream; список в меню (10) не совпадает с составом `extensions/` (17) | `src/configure.cpp:876,1231` |
| D6 | Ключи `ExtraArguments*` в `ytdl-patched.json` не читаются кодом | §2.5 |
| D7 | Четыре файла лицензии (`LICENSE`, `GPLv2`, `GPLv3`, `LICENSE.txt`) без пояснения, какой из них нормативный | корень репозитория |
| D8 | `QSettings` читается из рабочих потоков (`utils::qthread::run`, 15 мест) — потенциальные гонки | `src/*.cpp` |
| D9 | `//?????` в обработчиках завершения загрузки расширений (пустые `done()`/`failed()`) | `src/configure.cpp:983-987` |
| D10 | Самообновление приложения (Windows) переименовывает каталоги при старте — хрупкий и опасный путь | `src/utility.cpp:1358-1400` |

---

## 3. Функциональная карта

Легенда статуса: `MUST` — обязательна к переносу в первой полноценной версии; `SHOULD` — важна,
допускается во втором эшелоне; `LATER` — фиксируем в дорожной карте, не блокирует релиз;
`DROP?` — предлагается не переносить (решение владельца).

### 3.1 Загрузка и форматы

| ID | Функция | Реализация в оригинале | Зависимости | Приоритет | Критерий проверки |
| --- | --- | --- | --- | --- | --- |
| F-01 | Получение списка форматов по URL | `basicdownloader::list`, `engine::dumpJsonArguments` | yt-dlp `--print` JSON | MUST | для 3 контрольных URL список совпадает по составу полей с оригиналом |
| F-02 | Выбор формата / опций | `utility::args::parseOptions` (`src/utility.cpp:1548`), `OptionsArgument` = `-f` | движок | MUST | ввод `mp4`, `-f 22`, `default` даёт идентичный argv |
| F-03 | Загрузка одного URL | `basicdownloader::download` | движок, `--newline` | MUST | файл появляется в папке, прогресс идёт |
| F-04 | Прогресс и ETA | `Logger`, `ControlJsonStructure`, фильтры движков | stdout движка | MUST | проценты/скорость/ETA отображаются как в оригинале |
| F-05 | Отмена загрузки | `Terminator::terminate` | `taskkill`/`SIGTERM` | MUST | процесс и потомки завершены, состояние строки «cancelled» |
| F-06 | Пре-сеты опций (17 шт.) | `configure::presetOptions::defaultData` | — | MUST | предустановки совпадают по составу |
| F-07 | История введённых опций | `settings::addToHistory`, `HistorySize` | settings | SHOULD | история сохраняется и восстанавливается |

### 3.2 Пакетная загрузка

| ID | Функция | Реализация | Зависимости | Приоритет | Критерий |
| --- | --- | --- | --- | --- | --- |
| F-10 | Таблица URL + добавление | `batchdownloader::addToList` | — | MUST | строки добавляются, сохраняются |
| F-11 | Загрузка из файла (txt/json) | `batchdownloader::getListFromFile` | — | MUST | файл из оригинального формата читается |
| F-12 | Автозагрузка добавленного | `AutoDownloadWhenAddedInBatchDownloader`, `-a` | IPC | MUST | флаг работает |
| F-13 | Параллельные загрузки, лимит | `MaxConcurrentDownloads` (4) | — | MUST | лимит соблюдается, счётчики верны |
| F-14 | Метаданные/превью | `getMetaData`, `ShowMetaDataInBatchDownloader` | сеть, `QPixmap` | SHOULD | превью и заголовок заполняются |
| F-15 | Интервалы времени, главы, split | `--download-sections`, `cbSplitByChapters` | yt-dlp | SHOULD | аргументы совпадают |
| F-16 | Комментарии и субтитры | `--get-comments`, `--write-auto-subs` | yt-dlp | SHOULD | файлы сохраняются |
| F-17 | Сохранение/восстановление списка | `autoSavedList.json`, `AutoSavePlaylistOnExit` | — | SHOULD | список восстанавливается после перезапуска |
| F-18 | Запуск скачанного файла | `settings::mediaPlayer` (VLC/системный) | внешний плеер | SHOULD | файл открывается |
| F-19 | Переименование строки | `renameFile` | — | LATER | — |

### 3.3 Плейлисты и подписки

| ID | Функция | Реализация | Зависимости | Приоритет | Критерий |
| --- | --- | --- | --- | --- | --- |
| F-20 | Получение списка плейлиста | `playlistdownloader::parseJson`, `CanDownloadPlaylist` | yt-dlp | MUST | список совпадает с оригиналом |
| F-21 | Диапазон элементов | `--playlist-items`, `PlaylistItemsArgument` | движок | MUST | диапазон передаётся |
| F-22 | Выборочная/рекурсивная загрузка | `download`, `downloadRecursively` | — | MUST | состояние строк корректно |
| F-23 | Архив (`--download-archive`) | `enginePaths::archiveFileArgument` | файл архива | MUST | повторный запуск пропускает скачанное |
| F-24 | `--break-on-existing` / `--skip-on-existing` | `archiveData` (`src/utility.h:654`) | архив | SHOULD | поведение совпадает |
| F-25 | Подписки на плейлисты | `subscriptions.json`, `subscription::save` | архив, планировщик | SHOULD | добавление/удаление переживает перезапуск |
| F-26 | Автопроверка подписок | в оригинале — ручной запуск «Get List» | — | LATER | — |

### 3.4 Движки и расширения

| ID | Функция | Реализация | Зависимости | Приоритет | Критерий |
| --- | --- | --- | --- | --- | --- |
| F-30 | Чтение JSON расширения | `engines::engine` ctor (`src/engines.cpp:~1000-1120`) | файл | MUST | все 17 файлов из `extensions/` загружаются |
| F-31 | Выбор команды по ОС/архитектуре | `Cmd`, `addJsonCmd` | — | MUST | на каждой платформе выбрана верная команда |
| F-32 | Проверка версии движка | `VersionArgument`, `VersionStringLine/Position` | процесс | MUST | версия читается для yt-dlp, gallery-dl, wget |
| F-33 | Проверка минимальной версии | `RequiredMinimumVersionOfMediaDownloader` | — | MUST | старые расширения отвергаются с понятным сообщением |
| F-34 | Автообновление бинарников | `networkAccess::download`, `AutoUpdate` | сеть, GitHub API | MUST | обновление с проверкой SHA-256 |
| F-35 | Ручное добавление/удаление расширений | `configure` (Extensions) | сеть, архив | MUST | установка и удаление работают |
| F-36 | Готовые наборы (yt-dlp-aria2c, -ffmpeg, -nightly) | `extensions/*.json` + `configure.cpp` | — | SHOULD | наборы доступны в UI |
| F-37 | `LikeYoutubeDl` (семейство yt-dlp) | `engines::engine` | — | MUST | общий архив и парсер для семейства |
| F-38 | Поддерживающие движки (ffmpeg, deno, quickjs-ng, python, bsdtar) | `getSupportingEngineByName` (`src/engines.cpp:311`) | сеть | MUST | находятся/скачиваются как в оригинале |
| F-39 | Пользовательское расширение (`custom`) | `configure::addAction` | — | SHOULD | можно указать свой бинарник |
| F-40 | Режим проверки движка | `src/engines/tests.cpp` | сеть | LATER | — (в Rust заменяется тестами) |

### 3.5 Настройки, профили, маршрутизация URL

| ID | Функция | Реализация | Зависимости | Приоритет | Критерий |
| --- | --- | --- | --- | --- | --- |
| F-50 | Каталог загрузок, каталог библиотеки | `downloadFolder`, `LibraryDownloadFolder` | — | MUST | пути применяются |
| F-51 | Опции по умолчанию на движок | `downloadEngineDefaultOptions.json` | — | MUST | применяются при загрузке |
| F-52 | Url Manager (URL → движок+опции) | `downloadDefaultOptions.json`, `getEngineNameFromUrlManager` | — | MUST | правило срабатывает на подстроку |
| F-53 | Профили загрузки | **отдельной сущности нет**: ближайшие аналоги — пресеты (F-06), Url Manager (F-52), опции по умолчанию (F-51) | — | — | требуется решение: вводить ли профили как новую функцию или считать покрытой тремя механизмами |
| F-54 | Прокси: None/System/Env/Manual | `settings::proxySettings`, `proxy.cpp` | env/система | MUST | каждый режим даёт корректные аргументы/env |
| F-55 | Cookies из браузера / файла | `CookieArgument`, `CookieArgumentTextFile` | движок | MUST | аргументы совпадают |
| F-56 | Кодировка вывода | `EncodingArgument` | движок | SHOULD | не-UTF8 вывод читается |
| F-57 | Тема (тёмная/своя), UI Scale | `themes.cpp`, `HighDpiScalingFactor*` | — | SHOULD | темы на месте, масштаб применяется |
| F-58 | Уведомления и трей | `mainwindow.cpp:88-218` | системный трей | SHOULD | уведомление появляется |
| F-59 | Команда после загрузки / после всех | `CommandOnSuccessfulDownload`, `CommandWhenAllFinished` | shell | SHOULD | команда выполняется |
| F-60 | Язык интерфейса (18) | `translator.cpp`, `.qm` | — | MUST | все языки выбираются; для `ar` текст корректен, зеркалирования раскладки нет (D-002) |
| F-61 | Одиночный экземпляр + IPC | `single_instance.hpp`, `-u/-a/-e/-p` | сокет | MUST | второй экземпляр добавляет URL в первый |
| F-62 | Портативный режим (Windows) | `--portable`, `settings::init` | — | SHOULD | данные рядом с бинарником |
| F-63 | История загрузок | `downloadHistory.json`, окно лога | — | SHOULD | история пишется и читается |
| F-64 | Библиотека (файловый менеджер) | `library.cpp`, `directoryEntries.cpp` | ФС | SHOULD | навигация, переименование, удаление |
| F-65 | Самообновление приложения | `versionInfo`, `startedUpdatedVersion` | сеть | DROP? | требует решения владельца |

### 3.6 Платформы и сборка

| ID | Функция | Реализация | Приоритет | Критерий |
| --- | --- | --- | --- | --- |
| F-70 | Linux (X11/Wayland), Flatpak | CMake, flatpak manifest | MUST | сборка и запуск |
| F-71 | macOS (arm64, x86_64) bundle | CMake, `.icns`, plist | MUST | bundle запускается |
| F-72 | Windows 10/11 x86_64 | CMake + Inno Setup | MUST | установщик и portable |
| F-73 | Windows 7/8 x86/x64 | Qt5-сборка, `win7*` бинарники, quickjs-ng 0.17 | DROP (решено, D-005) | не переносится; причина зафиксирована в `docs/platforms.md` |
| F-74 | Иконки, `.desktop`, AppStream metainfo | `src/icons`, `src/*.desktop` | MUST | пакеты валидны |
| F-75 | CI-сборка трёх платформ + тесты | — (в оригинале тестов нет) | MUST | зелёный CI на 3 ОС |

---

## 4. Предварительная архитектура Rust-версии

Принцип: **сначала головное ядро без GUI, затем UI**. Оригинал смешивает их, из-за чего его нельзя
тестировать; повторять это в Rust нельзя.

```
morid/
├── crates/
│   ├── morid-core/        # домен: URL, движки, опции, состояния. Без I/O вообще.
│   ├── morid-engine/      # процесс движка: spawn, потоковый разбор stdout, отмена, прогресс
│   ├── morid-ext/         # схема расширения: чтение/валидация JSON, выбор команды по ОС/арх.
│   ├── morid-store/       # настройки, JSON-файлы данных, архив, импорт из Media Downloader
│   ├── morid-net/         # HTTP: релизы GitHub, загрузка бинарников, SHA-256, распаковка
│   ├── morid-i18n/        # каталоги строк + выбор языка
│   ├── morid-cli/         # bin: headless-режим и IPC-сервер (JSON-события)
│   └── morid-gui/         # bin (или lib для bin): UI
└── xtask/                 # задачи сборки/упаковки
```

Границы и обоснование:

| Компонент | Ответственность | Чего там быть не должно |
| --- | --- | --- |
| `morid-core` | типы: `MediaUrl`, `Format`, `DownloadOptions`, `EngineSpec`, `JobState`, `Error` | процессов, сети, файлов |
| `morid-engine` | запуск процесса, чтение stdout/stderr построчно, правила прогресса, отмена | знания про UI и про конкретный движок (только данные+правила) |
| `morid-ext` | разбор 34 ключей схемы, выбор `Cmd`, правила `ControlJsonStructure` | сетевых загрузок |
| `morid-store` | settings (INI-совместимо), `presetOptions.json`, `subscriptions.json`, архив, история | UI-состояния |
| `morid-net` | GitHub API, проверка digest, распаковка tar/zip | разбора вывода движков |
| `morid-cli` | headless: `morid download <url>`, `morid list <url>`, JSON-события, IPC | виджетов |

Ключевые архитектурные решения, которые предлагаются (каждое — с ценой):

1. **Движок = данные + правила, а не трейт с 57 методами.** Схема расширения остаётся данными;
   немногочисленные исключения (yt-dlp-подобные) — один-два специализированных парсера,
   выбираемых по флагу `LikeYoutubeDl`/имени, а не по одному классу на движок.
   Цена: если появится движок с принципиально иным поведением, потребуется явная точка расширения.
2. **Планировщик — один.** Очередь, лимит параллельности, состояния задач живут в `morid-core`,
   а вкладки GUI — только представления. Цена: переписывание логики трёх вкладок в одну модель.
3. **События вместо колбэков UI.** Ядро отдаёт поток событий (`JobProgress`, `JobFinished`,
   `EngineUpdated`, …); GUI и CLI — подписчики. Это же делает возможным headless-режим и
   тестируемость без GUI.
4. **Ошибки — типизированный `enum Error`** вместо строк с `ErrorCode=N`, распознаваемых подстрокой.
   Цена: тесты сравнения с оригиналом должны сопоставлять тексты, а не только коды.
5. **Совместимость данных — импорт, не совместное владение.** Читаем `settings.ini` и JSON-файлы
   оригинального приложения один раз и пишем свои. Цена: одноразовая миграция, которую надо
   протестировать на реальном каталоге пользователя.
6. **Отмена — дерево процессов.** На Unix — процесс-группа, на Windows — Job Object. Оригинал делает
   это частично (`taskkill`), что даёт «осиротевшие» дочерние процессы ffmpeg.

---

## 5. Предлагаемый технологический стек

Черновик; всё, что ниже, требует решения владельца (§9). **GUI-часть решена:** Slint (D-001).

| Слой | Вариант A (рекомендуется) | Вариант B | Вариант C |
| --- | --- | --- | --- |
| GUI | **Slint — принято владельцем (D-001)** | iced (MIT) | egui/eframe (MIT/Apache) |
| Асинхронность | потоки `std` + каналы, без runtime | tokio | async-std |
| JSON | `serde` + `serde_json` | — | — |
| HTTP | `ureq` (синхронный, небольшой) | `reqwest` + rustls | — |
| SHA-256 | `sha2` | — | — |
| Архивы | `tar` + `flate2`, `zip` | внешние `bsdtar` (как в оригинале) | — |
| INI | собственный парсер (~150 строк) | `rust-ini` | — |
| i18n | gettext `.po` (совместимо со Slint) | Fluent | собственные каталоги |
| Ошибки | `enum Error` + `thiserror`-совместимый `Display` вручную | `anyhow` (только в bin-крейтах) | — |
| Логи | собственный минимальный + `tracing` только в dev | — | — |

Замечания, которые обязан знать владелец:

- **RTL.** Арабский входит в 18 поддерживаемых языков, и в оригинале он работает через Qt
  (`setLayoutDirection`). В чисто-Rust тулкитах это слабое место: у iced RTL-раскладки нет, у egui —
  частично, у Slint поддержка направленности интерфейса ещё не завершена (upstream issue).
  **Решено (D-002):** первая версия без зеркалирования раскладки; ограничение публикуется в README и
  CHANGELOG, пункт «RTL-раскладка» уходит в дорожную карту без привязки к релизу.
- **Лицензия GUI.** Slint доступен по GPLv3 либо по royalty-free лицензии с указанием атрибуции.
  Morid — GPL-2.0-or-later, поэтому GPLv3-путь допустим («or later»), но фактически переводит
  распространяемые бинарники под GPLv3. Royalty-free-путь лицензию проекта не меняет. Нужно решение.
- **Размер.** Оригинал на Qt — десятки мегабайт с зависимостями. Целевой размер Rust-бинарника
  предлагается зафиксировать как измеримый бюджет (например, ≤ 20 MB с GUI, ≤ 4 MB headless) и
  проверять в CI, чтобы «минимализм» был измеримым, а не декларативным.
- **Зависимости.** Каждая новая зависимость проходит те же шесть вопросов, что в Tzim
  (`docs/dependencies.md`), с измеренной дельтой размера.

---

## 6. Риски миграции

| # | Риск | Вероятность | Влияние | Смягчение |
| --- | --- | --- | --- | --- |
| R1 | Нет эталона поведения: в оригинале нет тестов → «совместимость» нечем доказать | высокая | высокое | Фаза 0: построить harness дифференциальных проверок (одинаковый ввод → сравнение argv и разобранных событий) до переписывания |
| R2 | Разбор вывода сторонних CLI ломается при их обновлении | высокая | высокое | правила разбора — данные (`ControlJsonStructure`), у каждого правила тест на реальном сэмпле вывода |
| R3 | GUI-паритет: таблицы с превью, контекстные меню, трей, drag&drop | средняя | среднее | первый релиз — функциональный паритет, не пиксельный; список расхождений публикуется |
| R4 | RTL/локализация | средняя | среднее | принято (D-002): первая версия без зеркалирования; риск остаётся в виде недовольства арабоязычных пользователей |
| R5 | Совместимость со сторонними расширениями (34 ключа, 5 версий схемы, «мёртвые» ключи) | высокая | высокое | заморозить схему как публичный контракт с версионированием, включить все 17 файлов в набор тестов |
| R6 | `RequiredMinimumVersionOfMediaDownloader` привязывает расширения к версии продукта | высокая | среднее | правило совместимости для Morid (например, трактовать `2.x` как «совместимо») — требуется решение |
| R7 | Все сетевые адреса указывают на upstream (релизы, расширения по зашитому коммиту) | гарантирована | высокое | собственные конечные точки, зеркало `extensions/` внутри Morid, план на случай недоступности |
| R8 | Windows 7/8: поддержка требует старых тулчейнов и патчей зависимостей | снят (D-005) | — | поддержка не планируется, запись в дорожную карту и `docs/platforms.md` |
| R9 | Оценка объёма: 42,9 тыс. строк C++ плюс UI и 18 локализаций | гарантирована | высокое | вертикальные срезы по функциям, каждый — с критериями приёмки; не «переписать всё» |
| R10 | GPL-обязательства при переносе кода (производная работа) | гарантирована | высокое | сохранить GPL-2.0-or-later, уведомления об авторстве, THIRD-PARTY.md, юридическая проверка перед релизом |
| R11 | Дублирование функциональности: поддержка upstream-версии и Rust-версии параллельно | средняя | среднее | зафиксировать момент, когда C++-версия перестаёт быть источником новых функций |
| R12 | Отсутствие CI в форке | гарантирована | среднее | Фаза 1: CI (fmt/clippy/test/build) на 3 ОС до первого функционального кода |

---

## 7. Предлагаемая структура репозитория

```
Morid/
├── Cargo.toml                 # workspace
├── Cargo.lock
├── rust-toolchain.toml
├── README.md  SECURITY.md  CONTRIBUTING.md  CHANGELOG.md
├── CHANGELOG-legacy.md        # история оригинального Media Downloader, без правок
├── LICENSE                    # GPL-2.0 (текст), без изменений
├── LICENSE.txt  GPLv2  GPLv3  # уведомления об авторстве оригинала, без изменений
├── THIRD-PARTY.md
├── legacy/                    # C++-версия как эталон поведения (см. decisions.md D-006)
│   ├── README.md              # как собрать C++-версию для сверки
│   ├── src/  CMakeLists.txt  build_*.sh  build_windows.ps1  src/flatpak/  translations/
├── docs/
│   ├── audit-original.md      # аудит оригинала
│   ├── decisions.md           # журнал решений владельца (источник истины)
│   ├── owner-questions.md     # пояснения к решениям простым языком
│   ├── architecture.md
│   ├── roadmap.md
│   ├── migration.md
│   ├── features.md            # карта функций + статус миграции (из §3)
│   ├── extensions.md          # контракт схемы расширения, версии 1..4 (D-003)
│   ├── data-compat.md         # settings.ini и JSON-файлы оригинала (D-003)
│   ├── dependencies.md
│   ├── testing.md
│   ├── platforms.md           # матрица платформ (D-005)
│   ├── building.md
│   └── releases.md
├── crates/…                   # см. §4
├── ui/                        # .slint-файлы (D-001)
├── extensions/                # те же 17 JSON, версия схемы сохраняется (D-003)
├── i18n/                      # каталоги строк
├── assets/
│   ├── icons/                 # из src/icons (лицензия сохраняется)
│   └── desktop/               # .desktop, metainfo, иконки приложения
├── packaging/
│   ├── flatpak/  windows/  macos/  linux/
└── xtask/
```

C++-код переносится в `legacy/` (D-006) и сохраняется как эталон до того, как поведение будет
зафиксировано тестами. `extensions/`, лицензии и иконки остаются в корне: расширения — действующий
контракт (D-003), уведомления об авторстве не трогаются.

Документация опирается на структуру Tzim (README / SECURITY / CONTRIBUTING / CHANGELOG /
THIRD-PARTY / docs), но содержание — своё: у Morid нет «контрактов сохранения файлов», зато есть
контракт схемы расширения и контракт совместимости данных пользователя. Именно они занимают место
`guarantees.md` в Tzim.

---

## 8. План поэтапного переписывания

Каждая фаза заканчивается проверяемым результатом. Переход к следующей — только после приёмки.

**Фаза 0. Эталон поведения (без Rust-кода).**
Зафиксировать «золотые» данные оригинала: argv для набора сценариев (формат, опции, cookies, прокси,
плейлист, архив), реальные сэмплы вывода движков всех 10 используемых, разобранные в ожидаемые
события. Результат: набор фикстур в репозитории + скрипт сравнения. Без этого фазы 2+ не проверяемы.

**Фаза 1. Каркас.**
Workspace, CI (3 ОС: fmt, clippy `-D warnings`, test, build, измерение размера бинарника),
лицензионные файлы, README/SECURITY/CONTRIBUTING/CHANGELOG, `docs/dependencies.md` с процедурой
приёмки зависимостей. Результат: пустой headless-бинарник со зелёным CI.

**Фаза 2. Ядро домена + схема расширения.**
`morid-core` + `morid-ext`: все 17 файлов из `extensions/` читаются, валидируются, команда выбирается
по ОС/архитектуре; правила `ControlJsonStructure` реализованы как данные. Без сети и без процессов.
Результат: тесты на 17 файлов + негативные (битый JSON, отсутствующий `Name`, старая версия схемы).

**Фаза 3. Один движок end-to-end (yt-dlp, headless).**
`morid-engine` + `morid-cli`: список форматов и загрузка одного URL, прогресс в JSON-событиях,
отмена с деревом процессов. Сверка с эталоном Фазы 0. Результат: `morid list/download` работает и
сравним с оригиналом.

**Фаза 4. Данные пользователя.**
`morid-store`: settings INI (совместимое чтение), пресеты, Url Manager, опции по умолчанию, архивы,
история, подписки; одноразовый импорт из каталога Media Downloader. Результат: реальный каталог
пользователя импортируется без потерь, проверено на копии.

**Фаза 5. Сеть и обновления движков.**
`morid-net`: релизы, SHA-256, распаковка, установка/удаление расширений, поддерживающие движки.
Собственные конечные точки вместо upstream. Результат: установка и обновление yt-dlp без
вмешательства пользователя, отказ при несовпадении хэша.

**Фаза 6. Остальные движки.**
gallery-dl, you-get, svtplay-dl, get-sauce, wget, aria2c, python/ffmpeg/deno/quickjs-ng — по одному,
каждый с фикстурами вывода. Результат: паритет по списку функций §3.

**Фаза 7. GUI — минимальный вертикальный срез.**
Окно, вкладка Basic Downloader, список форматов, прогресс, настройки каталога. Затем Batch, затем
Playlist, затем Library, затем Configure. Локализация — вместе с каждым экраном, а не «в конце».

**Фаза 8. Паритет и упаковка.**
Трей, уведомления, темы, RTL, библиотека, история загрузок, IPC/одиночный экземпляр; пакеты
(Flatpak, macOS bundle, Windows installer/portable), миграция пользователей, публикация.

Порядок «сначала headless» выбран потому, что он даёт проверяемость на фазах 2–6 и позволяет
выпустить полезный инструмент до готовности GUI.

---

## 9. Вопросы, требующие решения владельца

**Решённые вопросы (4 ноября 2026) вынесены в журнал `docs/decisions.md`:** Q1 — Slint (D-001);
Q2 — RTL откладывается (D-002); Q5 — полная совместимость (D-003); Q6 — мягкая проверка версий
(D-004, как следствие D-003); Q7 — только Windows 10/11 (D-005); Q11 — этот форк, C++ в `legacy/`
(D-006). Ниже таблица сохранена как исходная постановка; актуальный статус — в журнале решений.

| # | Вопрос | Варианты | Что блокирует |
| --- | --- | --- | --- |
| Q1 | Целевой GUI-тулкит | (a) Slint — декларативный, Rust-first, лицензионные варианты; (b) iced — MIT, но нет RTL; (c) egui — быстрый старт, слабая доступность; (d) сохранить Qt через C++-мост — противоречит Rust-first | всю Фазу 7, оценку объёма |
| Q2 | RTL (арабский) в первой версии | (a) обязателен — влияет на выбор тулкита; (b) переносим в дорожную карту, временно без зеркалирования | Q1 |
| Q3 | Лицензионный путь Slint, если выбран Slint | (a) royalty-free + атрибуция (лицензия проекта не меняется); (b) GPLv3 (бинарники под GPLv3) | юридическую чистоту |
| Q4 | Судьба C++-кода в дереве | (a) перенести в `legacy/` как эталон; (b) удалить после фиксации эталона; (c) оставить на месте | структуру репозитория |
| Q5 | Совместимость с существующими расширениями и файлами данных | (a) полная: те же 34 ключа, импорт settings.ini и JSON; (b) только свои форматы, импорт не поддерживаем | Фазы 2, 4, 5 |
| Q6 | `RequiredMinimumVersionOfMediaDownloader` в чужих расширениях (значения `2.x`) | (a) трактовать как совместимые; (b) требовать перевыпуска расширений под Morid | Фазу 2 |
| Q7 | Windows 7/8 | (a) отказаться, поддерживать Windows 10+; (b) поддерживать, что требует старых тулчейнов и патчей | платформы, размер работы |
| Q8 | Самообновление приложения (F-65) | (a) не переносить; (b) перенести в последнюю очередь; (c) сделать через пакетные менеджеры | Фазу 8, безопасность |
| Q9 | Вводить ли «профили загрузки» (F-53) как новую сущность | (a) нет, покрыто пресетами + Url Manager + опциями по умолчанию; (b) да, новая функция в Rust-версии | объём функциональности |
| Q10 | Язык документации проекта | (a) английский (как Tzim); (b) русский; (c) английский публично + русские рабочие документы | все документы |
| Q11 | Где живёт продукт: этот форк или новый репозиторий с чистой историей | (a) этот форк, C++ в `legacy/`; (b) новый репозиторий, форк остаётся эталоном | структуру и историю |
| Q12 | Целевой размер бинарника (бюджет) | (a) зафиксировать бюджет и проверять в CI; (b) не ограничивать | принцип «минимализм» |
| Q13 | Поддержка headless-режима как продуктовой функции | (a) да, отдельный бинарник `morid`; (b) только внутренний инструмент для тестов | архитектуру, фазы 2–6 |
| Q14 | Источник расширений после отказа от upstream-коммита | (a) файлы в репозитории Morid как единственный источник; (b) свой URL с версионированием; (c) оба, с приоритетом локальных | Фазу 5 |
| Q15 | Формат сетевых конечных точек для обновления движков (сейчас — `api.github.com` проектов upstream) | (a) собственные адреса Morid; (b) оставить upstream-адреса движков (yt-dlp и др. — чужие проекты и адреса менять не нужно), сменить только адреса самого приложения и расширений | Фазу 5, риск R7 |

---

## 10. Что дальше

1. ~~Владелец отвечает на Q1–Q13~~ — сделано 4 ноября 2026: пять блокирующих вопросов решены
   (`docs/decisions.md`, D-001…D-006). Остаются открытыми Q3, Q4, Q8, Q9, Q10, Q12, Q13 и новые
   Q14, Q15.
2. Оформляются `docs/architecture.md`, `docs/roadmap.md`, `docs/features.md` (карта §3 с
   приоритетами и статусами), `docs/migration.md`, `docs/extensions.md`, `docs/data-compat.md`,
   `docs/platforms.md` — с учётом принятых решений.
3. Первое задание кодеру — **Фаза 0** (эталон поведения и фикстуры) и **Фаза 1** (каркас + CI),
   двумя отдельными задачами. Задание на переписывание функциональности не выдаётся до завершения
   аудита и согласования архитектуры.
