#pragma once

#include <QMainWindow>
#include <QSplitter>
#include <QStatusBar>
#include <QToolBar>
#include <QLabel>
#include <memory>

class IntercomClient;
class ChannelListWidget;
class UserListWidget;
class TalkButtonWidget;

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow();

protected:
    void keyPressEvent(QKeyEvent *event) override;
    void keyReleaseEvent(QKeyEvent *event) override;

private slots:
    void onConnectClicked();
    void onDisconnectClicked();
    void onSettingsClicked();
    void onConnectionStateChanged(int state);
    void onTalkStateChanged(bool talking);
    void onError(const QString &message);

private:
    void setupUI();
    void setupToolbar();
    void setupStatusBar();
    void updateConnectionState(bool connected);

    // UI Components
    QSplitter *m_splitter;
    ChannelListWidget *m_channelList;
    UserListWidget *m_userList;
    TalkButtonWidget *m_talkButton;

    // Status bar components
    QLabel *m_connectionStatus;
    QLabel *m_talkingStatus;

    // Toolbar actions
    QAction *m_connectAction;
    QAction *m_disconnectAction;
    QAction *m_settingsAction;

    // Intercom client
    std::unique_ptr<IntercomClient> m_client;

    // State
    bool m_spaceBarPressed = false;
};
