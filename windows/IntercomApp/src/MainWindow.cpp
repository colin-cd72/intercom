#include "MainWindow.h"
#include "IntercomClient.h"
#include "ChannelListWidget.h"
#include "UserListWidget.h"
#include "TalkButtonWidget.h"
#include "DeviceSettingsDialog.h"
#include "ConnectDialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QKeyEvent>
#include <QMessageBox>
#include <QToolBar>
#include <QStatusBar>

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
{
    setWindowTitle("Intercom");
    setMinimumSize(900, 600);
    resize(1100, 700);

    setupUI();
    setupToolbar();
    setupStatusBar();

    // Create intercom client
    m_client = std::make_unique<IntercomClient>(this);

    // Connect signals
    connect(m_client.get(), &IntercomClient::connectionStateChanged,
            this, &MainWindow::onConnectionStateChanged);
    connect(m_client.get(), &IntercomClient::talkStateChanged,
            this, &MainWindow::onTalkStateChanged);
    connect(m_client.get(), &IntercomClient::errorOccurred,
            this, &MainWindow::onError);

    // Connect talk button
    connect(m_talkButton, &TalkButtonWidget::talkStarted,
            m_client.get(), &IntercomClient::startTalk);
    connect(m_talkButton, &TalkButtonWidget::talkStopped,
            m_client.get(), &IntercomClient::stopTalk);

    updateConnectionState(false);
}

MainWindow::~MainWindow() = default;

void MainWindow::setupUI()
{
    QWidget *centralWidget = new QWidget(this);
    setCentralWidget(centralWidget);

    QHBoxLayout *mainLayout = new QHBoxLayout(centralWidget);
    mainLayout->setContentsMargins(0, 0, 0, 0);
    mainLayout->setSpacing(0);

    // Create splitter for resizable panels
    m_splitter = new QSplitter(Qt::Horizontal, this);

    // Left panel - Channel list
    m_channelList = new ChannelListWidget(this);
    m_channelList->setMinimumWidth(180);
    m_channelList->setMaximumWidth(300);
    m_splitter->addWidget(m_channelList);

    // Center panel - Talk button
    QWidget *centerPanel = new QWidget(this);
    QVBoxLayout *centerLayout = new QVBoxLayout(centerPanel);
    centerLayout->setContentsMargins(20, 20, 20, 20);
    centerLayout->setAlignment(Qt::AlignCenter);

    m_talkButton = new TalkButtonWidget(this);
    centerLayout->addWidget(m_talkButton, 0, Qt::AlignCenter);

    m_splitter->addWidget(centerPanel);

    // Right panel - User list
    m_userList = new UserListWidget(this);
    m_userList->setMinimumWidth(180);
    m_userList->setMaximumWidth(300);
    m_splitter->addWidget(m_userList);

    // Set splitter sizes
    m_splitter->setSizes({200, 500, 200});

    mainLayout->addWidget(m_splitter);
}

void MainWindow::setupToolbar()
{
    QToolBar *toolbar = addToolBar("Main");
    toolbar->setMovable(false);
    toolbar->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);

    m_connectAction = toolbar->addAction(
        style()->standardIcon(QStyle::SP_ComputerIcon),
        "Connect",
        this, &MainWindow::onConnectClicked
    );

    m_disconnectAction = toolbar->addAction(
        style()->standardIcon(QStyle::SP_DialogCloseButton),
        "Disconnect",
        this, &MainWindow::onDisconnectClicked
    );
    m_disconnectAction->setEnabled(false);

    toolbar->addSeparator();

    m_settingsAction = toolbar->addAction(
        style()->standardIcon(QStyle::SP_FileDialogDetailedView),
        "Audio Settings",
        this, &MainWindow::onSettingsClicked
    );
}

void MainWindow::setupStatusBar()
{
    QStatusBar *status = statusBar();

    m_connectionStatus = new QLabel("Disconnected");
    status->addWidget(m_connectionStatus);

    status->addWidget(new QLabel(" | "));

    m_talkingStatus = new QLabel("");
    status->addWidget(m_talkingStatus);
}

void MainWindow::keyPressEvent(QKeyEvent *event)
{
    if (event->key() == Qt::Key_Space && !event->isAutoRepeat() && !m_spaceBarPressed) {
        m_spaceBarPressed = true;
        m_talkButton->setPressed(true);
    }
    QMainWindow::keyPressEvent(event);
}

void MainWindow::keyReleaseEvent(QKeyEvent *event)
{
    if (event->key() == Qt::Key_Space && !event->isAutoRepeat() && m_spaceBarPressed) {
        m_spaceBarPressed = false;
        m_talkButton->setPressed(false);
    }
    QMainWindow::keyReleaseEvent(event);
}

void MainWindow::onConnectClicked()
{
    ConnectDialog dialog(this);
    if (dialog.exec() == QDialog::Accepted) {
        m_client->connect(dialog.roomId(), dialog.displayName());
    }
}

void MainWindow::onDisconnectClicked()
{
    m_client->disconnect();
}

void MainWindow::onSettingsClicked()
{
    DeviceSettingsDialog dialog(this);
    dialog.exec();
}

void MainWindow::onConnectionStateChanged(int state)
{
    bool connected = (state == 2); // Connected state
    updateConnectionState(connected);

    QString statusText;
    switch (state) {
    case 0: statusText = "Disconnected"; break;
    case 1: statusText = "Connecting..."; break;
    case 2: statusText = "Connected"; break;
    case 3: statusText = "Disconnecting..."; break;
    case 4: statusText = "Connection Failed"; break;
    default: statusText = "Unknown"; break;
    }
    m_connectionStatus->setText(statusText);
}

void MainWindow::onTalkStateChanged(bool talking)
{
    m_talkingStatus->setText(talking ? "Transmitting" : "");
    m_talkButton->setTalking(talking);
}

void MainWindow::onError(const QString &message)
{
    QMessageBox::warning(this, "Error", message);
}

void MainWindow::updateConnectionState(bool connected)
{
    m_connectAction->setEnabled(!connected);
    m_disconnectAction->setEnabled(connected);
    m_talkButton->setEnabled(connected);
}
