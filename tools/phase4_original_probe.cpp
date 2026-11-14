// Readback-only test instrumentation, loaded into an unmodified legacy build.
// This is not a fixture capture driver and never writes fixtures.
#include <QAction>
#include <QApplication>
#include <QComboBox>
#include <QCoreApplication>
#include <QLineEdit>
#include <QMenu>
#include <QPushButton>
#include <QSettings>
#include <QTabWidget>
#include <QTableWidget>
#include <QTextStream>
#include <QTimer>
#include <QWidget>
#include <dlfcn.h>
#include <functional>

static void message(const QString& value) { QTextStream(stdout) << value << Qt::endl; }
static void fail(const QString& value) { message("ORIGINAL_READBACK_FAIL " + value); QCoreApplication::exit(2); }
static QWidget* window() {
    for (auto w : QApplication::topLevelWidgets()) if (w->findChild<QTabWidget*>("tabWidget")) return w;
    return nullptr;
}
template<class T> static T* target(const char* name) { auto w = window(); return w ? w->findChild<T*>(name) : nullptr; }
static bool text(const char* name, const QString& expected) {
    auto w = target<QLineEdit>(name);
    if (!w || w->text() != expected) { fail(QString("%1 expected=%2 actual=%3").arg(name, expected, w ? w->text() : "MISSING")); return false; }
    message(QString("ORIGINAL_UI %1=%2").arg(name, w->text())); return true;
}
static bool table(const char* name, int expected) {
    auto w = target<QTableWidget>(name);
    if (!w || w->rowCount() != expected) { fail(QString("%1 expected rows=%2 actual=%3").arg(name).arg(expected).arg(w ? w->rowCount() : -1)); return false; }
    message(QString("ORIGINAL_UI %1 rows=%2").arg(name).arg(w->rowCount()));
    for (int r=0; r<w->rowCount(); ++r) {
        QStringList cells;
        for (int c=0; c<w->columnCount(); ++c) if (auto item=w->item(r,c)) cells << item->text();
        message(QString("ORIGINAL_ROW %1 %2 %3").arg(name).arg(r).arg(cells.join(" | ")));
    }
    return true;
}
static void verifySubscription() {
    if (!table("tableWidgetPlDownloaderSubscription", 1)) return;
    auto w=target<QTableWidget>("tableWidgetPlDownloaderSubscription");
    if (!w->item(0,0) || w->item(0,0)->text() != "Example subscription" ||
        !w->item(0,1) || w->item(0,1)->text() != "https://example.com/playlist/subscription") {
        fail("subscription contents differ"); return;
    }
    message("ORIGINAL_ROUNDTRIP_OK settings + QStringList(21) + presets(15) + URL rule(1) + engine defaults(2) + saved list(2) + subscription(1)");
    QCoreApplication::exit(0);
}
static void verifyConfigure() {
    if (!text("lineEditConfigureDownloadPath", "/home/user") ||
        !text("lineEditConfigureTextEncoding", "ISO-8859-1") ||
        !text("lineEditConfigureMaximuConcurrentDownloads", "1") ||
        !table("tableWidgetConfigurePresetOptions", 15) ||
        !table("tableWidgetConfigureUrl", 1) ||
        !table("tableWidgetEnginesDefaultOptions", 2)) return;
    target<QTabWidget>("tabWidget")->setCurrentIndex(2);
    QTimer::singleShot(400, [] {
        auto button=target<QPushButton>("pbPlSubscription");
        if (!button) { fail("subscription button missing"); return; }
        QTimer::singleShot(150, [] {
            for (auto w : QApplication::topLevelWidgets()) {
                auto menu=qobject_cast<QMenu*>(w);
                if (!menu || !menu->isVisible()) continue;
                for (auto action : menu->actions()) if (action->text() == "Manage Subscriptions") {
                    action->trigger(); menu->close();
                    QTimer::singleShot(400, verifySubscription); return;
                }
            }
            fail("Manage Subscriptions action missing");
        });
        button->click();
    });
}
static void startProbe(int attempt=0) {
    auto tabs=target<QTabWidget>("tabWidget");
    auto engine=target<QComboBox>("cbConfigureEngines");
    if (!tabs || !engine || engine->findText("yt-dlp") < 0) {
        if (attempt >= 180) { fail("legacy UI did not initialize"); return; }
        QTimer::singleShot(500, [attempt] { startProbe(attempt+1); }); return;
    }
    QSettings settings(qEnvironmentVariable("MediaDownloaderAppDataPath") + "/settings/settings.ini", QSettings::IniFormat);
    const auto history=settings.value("BasicDownloaderOptionsHistory-yt-dlp").toStringList();
    message(QString("ORIGINAL_QT version=%1 keys=%2 history_items=%3").arg(qVersion()).arg(settings.allKeys().size()).arg(history.size()));
    if (history.size()!=21 || history.first()!="--history-option 20" || history.last()!="--history-option") {
        fail("QStringList changed"); return;
    }
    if (!table("tableWidgetBD", 2)) return;
    tabs->setCurrentIndex(4);
    engine->setCurrentText("yt-dlp");
    auto rules=target<QComboBox>("cbConfigureEnginesUrlManager");
    if (!rules || rules->findText("gallery-dl") < 0) { fail("URL-rule engine missing"); return; }
    rules->setCurrentText("gallery-dl");
    QTimer::singleShot(500, verifyConfigure);
}
extern "C" int _ZN16QCoreApplication4execEv() {
    using Exec=int(*)();
    auto real=reinterpret_cast<Exec>(dlsym(RTLD_NEXT,"_ZN16QCoreApplication4execEv"));
    if (!real) return 127;
    QTimer::singleShot(2000, [] { startProbe(); });
    QTimer::singleShot(120000, [] { fail("readback deadline exceeded"); });
    return real();
}
