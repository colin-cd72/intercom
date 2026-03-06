#pragma once

#include <QObject>
#include <QString>
#include <QVector>
#include <memory>

struct AudioDeviceInfo {
    QString name;
    bool isDante;
    bool isDefault;
};

struct ChannelInfo {
    QString id;
    QString name;
    QString description;
};

struct UserInfo {
    QString id;
    QString displayName;
    bool isTalking;
};

class IntercomClient : public QObject
{
    Q_OBJECT

public:
    enum ConnectionState {
        Disconnected = 0,
        Connecting = 1,
        Connected = 2,
        Disconnecting = 3,
        Failed = 4
    };
    Q_ENUM(ConnectionState)

    explicit IntercomClient(QObject *parent = nullptr);
    ~IntercomClient();

    // Device management
    QVector<AudioDeviceInfo> inputDevices() const;
    QVector<AudioDeviceInfo> outputDevices() const;
    void setInputDevice(const QString &name);
    void setOutputDevice(const QString &name);
    void setPreferDante(bool prefer);
    bool preferDante() const { return m_preferDante; }

    // Connection
    void connect(const QString &roomId, const QString &displayName);
    void disconnect();
    bool isConnected() const { return m_connectionState == Connected; }
    ConnectionState connectionState() const { return m_connectionState; }
    QString roomId() const { return m_roomId; }

    // Channels
    void subscribeChannel(const QString &channelId);
    void unsubscribeChannel(const QString &channelId);
    QVector<ChannelInfo> channels() const { return m_channels; }

    // Users
    QVector<UserInfo> users() const { return m_users; }

    // Talking
    void startTalk();
    void stopTalk();
    bool isTalking() const { return m_isTalking; }
    void setTalkChannel(const QString &channelId);
    QString talkChannel() const { return m_talkChannel; }

    // Audio levels
    float inputLevel() const { return m_inputLevel; }
    float outputLevel() const { return m_outputLevel; }

signals:
    void connectionStateChanged(int state);
    void channelsChanged();
    void usersChanged();
    void userJoined(const UserInfo &user);
    void userLeft(const QString &userId);
    void userTalkStart(const QString &userId, const QString &channelId);
    void userTalkStop(const QString &userId);
    void talkStateChanged(bool talking);
    void audioLevelsChanged(float input, float output);
    void errorOccurred(const QString &message);

private:
    void refreshDevices();

    // FFI handle (placeholder - will hold actual Rust client)
    void *m_ffiClient = nullptr;

    // State
    ConnectionState m_connectionState = Disconnected;
    QString m_roomId;
    QString m_displayName;
    QString m_talkChannel;
    bool m_isTalking = false;
    bool m_preferDante = true;
    float m_inputLevel = 0.0f;
    float m_outputLevel = 0.0f;

    // Cached data
    QVector<AudioDeviceInfo> m_inputDevices;
    QVector<AudioDeviceInfo> m_outputDevices;
    QVector<ChannelInfo> m_channels;
    QVector<UserInfo> m_users;
    QString m_selectedInputDevice;
    QString m_selectedOutputDevice;
};
