// Development-only GUI driver for Phase 0 captures.  It is loaded with
// LD_PRELOAD; no file in legacy/ is changed or linked into this helper.
#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QComboBox>
#include <QCoreApplication>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLineEdit>
#include <QMenu>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QTabWidget>
#include <QTableWidget>
#include <QTextStream>
#include <QTimer>
#include <QWidget>

#include <dlfcn.h>

#include <cstdlib>

static QTextStream& captureOut()
{
    static QTextStream out( stdout );
    return out;
}

class CaptureDriver : public QObject
{
public:
    CaptureDriver()
    {
        const auto path = qEnvironmentVariable( "CAPTURE_STEPS" );
        const auto errorPath = qEnvironmentVariable( "CAPTURE_DRIVER_ERROR" );

        m_errorPath = errorPath;

        QFile file( path );
        if( !file.open( QIODevice::ReadOnly ) ) {
            fail( "cannot open steps file: " + path );
            return;
        }

        const auto document = QJsonDocument::fromJson( file.readAll() );
        if( !document.isArray() ) {
            fail( "steps file is not a JSON array" );
            return;
        }

        m_steps = document.array();
        m_settleMs = qEnvironmentVariableIntValue( "CAPTURE_SETTLE_MS" );
        if( m_settleMs <= 0 ) {
            m_settleMs = 2000;
        }

        QTimer::singleShot( 250, this, [ this ]() { tick(); } );
    }

private:
    QWidget* mainWindow()
    {
        for( auto widget : QApplication::topLevelWidgets() ) {
            if( widget->findChild<QTabWidget*>( "tabWidget" ) ) {
                return widget;
            }
        }
        return nullptr;
    }

    template< typename T >
    T* target( const QString& name )
    {
        auto window = mainWindow();
        return window ? window->findChild<T*>( name ) : nullptr;
    }

    void fail( const QString& message )
    {
        captureOut() << "capture-driver ERROR: " << message << Qt::endl;
        captureOut().flush();

        if( !m_errorPath.isEmpty() ) {
            QFile file( m_errorPath );
            if( file.open( QIODevice::WriteOnly | QIODevice::Truncate ) ) {
                file.write( message.toUtf8() );
                file.write( "\n" );
            }
        }

        dumpUiLog();
        QCoreApplication::exit( 2 );
    }

    void dumpUiLog()
    {
        const auto path = qEnvironmentVariable( "CAPTURE_UI_LOG" );
        if( path.isEmpty() ) {
            return;
        }

        QFile file( path );
        if( !file.open( QIODevice::WriteOnly | QIODevice::Truncate ) ) {
            return;
        }

        if( auto logger = target<QPlainTextEdit>( "plainTextEditLogger" ) ) {
            file.write( logger->toPlainText().toUtf8() );
            file.write( "\n" );
        }

        for( auto widget : QApplication::topLevelWidgets() ) {
            for( auto logger : widget->findChildren<QPlainTextEdit*>() ) {
                if( logger->objectName() != "plainTextEditLogger" ) {
                    continue;
                }
                file.write( logger->toPlainText().toUtf8() );
                file.write( "\n" );
            }
        }
    }

    void selectMenuAction( const QString& value )
    {
        QWidget* popup = QApplication::activePopupWidget();
        if( !popup ) {
            for( auto candidate : QApplication::topLevelWidgets() ) {
                if( qobject_cast<QMenu*>( candidate ) && candidate->isVisible() ) {
                    popup = candidate;
                    break;
                }
            }
        }
        if( !popup ) {
            fail( "menu did not open" );
            return;
        }

        for( auto action : popup->findChildren<QAction*>() ) {
            if( action->text() == value ) {
                if( !action->isEnabled() ) {
                    fail( "menu action is disabled: " + value );
                    return;
                }
                action->trigger();
                popup->close();
                return;
            }
        }
        fail( "menu action not found: " + value );
    }

    void finish()
    {
        dumpUiLog();
        captureOut() << "capture-driver: completed " << m_steps.size() << " steps" << Qt::endl;
        captureOut().flush();
        QCoreApplication::quit();
    }

    void tick()
    {
        if( !mainWindow() ) {
            if( ++m_waitCount > 240 ) {
                fail( "main window did not appear" );
            } else {
                QTimer::singleShot( 250, this, [ this ]() { tick(); } );
            }
            return;
        }

        if( m_index == 0 ) {
            // Let tabManager::init_done() and queued startup events run first.
            if( m_waitCount++ < 4 ) {
                QTimer::singleShot( 250, this, [ this ]() { tick(); } );
                return;
            }
        }

        if( m_index >= m_steps.size() ) {
            if( !m_settling ) {
                m_settling = true;
                QTimer::singleShot( m_settleMs, this, [ this ]() { finish(); } );
            }
            return;
        }

        const auto step = m_steps.at( m_index ).toObject();
        const auto action = step.value( "action" ).toString();
        const auto name = step.value( "target" ).toString();
        const auto value = step.value( "value" ).toString();

        captureOut() << "capture-driver: " << action << " " << name << Qt::endl;

        if( action == "sleep" ) {
            ++m_index;
            QTimer::singleShot( step.value( "ms" ).toInt( 250 ), this, [ this ]() { tick(); } );
            return;
        }

        if( action == "tab" ) {
            auto widget = target<QTabWidget>( name );
            if( !widget ) {
                fail( "tab widget not found: " + name );
                return;
            }
            widget->setCurrentIndex( step.value( "index" ).toInt() );
        } else if( action == "text" ) {
            auto widget = target<QLineEdit>( name );
            if( !widget ) {
                fail( "line edit not found: " + name );
                return;
            }
            widget->setText( value );
        } else if( action == "plainText" ) {
            auto widget = target<QPlainTextEdit>( name );
            if( !widget ) {
                fail( "plain text edit not found: " + name );
                return;
            }
            widget->setPlainText( value );
        } else if( action == "combo" ) {
            auto widget = target<QComboBox>( name );
            if( !widget ) {
                fail( "combo box not found: " + name );
                return;
            }
            if( widget->findText( value ) == -1 ) {
                QStringList values;
                for( int i = 0; i < widget->count(); ++i ) {
                    values.append( widget->itemText( i ) );
                }
                fail( "combo value " + value + " not found in " + name + ": " + values.join( ", " ) );
                return;
            }
            widget->setCurrentText( value );
        } else if( action == "check" ) {
            auto widget = target<QCheckBox>( name );
            if( !widget ) {
                fail( "check box not found: " + name );
                return;
            }
            widget->setChecked( step.value( "checked" ).toBool() );
        } else if( action == "clickMenu" ) {
            auto widget = target<QPushButton>( name );
            if( !widget ) {
                fail( "menu push button not found: " + name );
                return;
            }
            // QMenu::exec() blocks widget->click(). Select its action from a
            // timer running in the menu's nested event loop.
            QTimer::singleShot( 250, this, [ this, value ]() { selectMenuAction( value ); } );
            widget->click();
        } else if( action == "click" ) {
            auto widget = target<QPushButton>( name );
            if( !widget ) {
                fail( "push button not found: " + name );
                return;
            }
            if( !widget->isEnabled() ) {
                fail( "push button is disabled: " + name );
                return;
            }
            widget->click();
        } else if( action == "selectRow" ) {
            auto widget = target<QTableWidget>( name );
            if( !widget ) {
                fail( "table not found: " + name );
                return;
            }
            const auto row = step.value( "row" ).toInt();
            if( row < 0 || row >= widget->rowCount() ) {
                fail( QString( "row %1 outside table %2 with %3 rows" ).arg( row ).arg( name ).arg( widget->rowCount() ) );
                return;
            }
            widget->selectRow( row );
            widget->setCurrentCell( row, 0 );
        } else if( action == "contextMenu" ) {
            auto widget = target<QTableWidget>( name );
            if( !widget ) {
                fail( "table not found for context menu: " + name );
                return;
            }

            QTimer::singleShot( 250, this, [ this, value ]() { selectMenuAction( value ); } );

            QMetaObject::invokeMethod( widget, "customContextMenuRequested", Q_ARG( QPoint, QPoint( 0, 0 ) ) );
        } else {
            fail( "unknown action: " + action );
            return;
        }

        ++m_index;
        QTimer::singleShot( step.value( "delay" ).toInt( 150 ), this, [ this ]() { tick(); } );
    }

    QJsonArray m_steps;
    QString m_errorPath;
    int m_index = 0;
    int m_waitCount = 0;
    int m_settleMs = 2000;
    bool m_settling = false;
};

// QApplication::exec() resolves to this static QCoreApplication symbol.
// Interposition only schedules the driver and then calls Qt's real exec().
extern "C" int _ZN16QCoreApplication4execEv()
{
    using Exec = int ( * )();
    static auto realExec = reinterpret_cast<Exec>( dlsym( RTLD_NEXT, "_ZN16QCoreApplication4execEv" ) );

    if( !realExec ) {
        return 127;
    }

    if( qEnvironmentVariableIsSet( "CAPTURE_STEPS" ) ) {
        static CaptureDriver* driver = new CaptureDriver();
        Q_UNUSED( driver );
    }

    return realExec();
}
