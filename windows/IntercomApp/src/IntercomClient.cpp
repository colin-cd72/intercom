#include "IntercomClient.h"
#include <QDebug>
#include <QTimer>

IntercomClient::IntercomClient(QObject *parent)
    : QObject(parent)
{
    refreshDevices();
}

IntercomClient::~IntercomClient()
{
    if (m_connectionState == Connected) {
        disconnect();
    }
}

void IntercomClient::refreshDevices()
{
    // TODO: Call Rust FFI to get actual devices
    // For now, use mock data

    m_inputDevices.clear();
    m_inputDevices.append({"Dante Virtual Soundcard", true, false});
    m_inputDevices.append({"Microphone (Realtek Audio)", false, true});
    m_inputDevices.append({"USB Microphone", false, false});

    m_outputDevices.clear();
    m_outputDevices.append({"Dante Virtual Soundcard", true, false});
    m_outputDevices.append({"Speakers (Realtek Audio)", false, true});
    m_outputDevices.append({"Headphones", false, false});

    // Auto-select DANTE if preferred
    if (m_preferDante) {
        for (const auto &device : m_inputDevices) {
            if (device.isDante) {
                m_selectedInputDevice = device.name;
                break;
            }
        }
        for (const auto &device : m_outputDevices) {
            if (device.isDante) {
                m_selectedOutputDevice = device.name;
                break;
            }
        }
    }
}

QVector<AudioDeviceInfo> IntercomClient::inputDevices() const
{
    return m_inputDevices;
}

QVector<AudioDeviceInfo> IntercomClient::outputDevices() const
{
    return m_outputDevices;
}

void IntercomClient::setInputDevice(const QString &name)
{
    m_selectedInputDevice = name;
    // TODO: Call Rust FFI to set device
    qDebug() << "Set input device:" << name;
}

void IntercomClient::setOutputDevice(const QString &name)
{
    m_selectedOutputDevice = name;
    // TODO: Call Rust FFI to set device
    qDebug() << "Set output device:" << name;
}

void IntercomClient::setPreferDante(bool prefer)
{
    m_preferDante = prefer;
    refreshDevices();
}

void IntercomClient::connect(const QString &roomId, const QString &displayName)
{
    if (m_connectionState != Disconnected) {
        emit errorOccurred("Already connecting or connected");
        return;
    }

    m_roomId = roomId;
    m_displayName = displayName;
    m_connectionState = Connecting;
    emit connectionStateChanged(m_connectionState);

    // TODO: Call Rust FFI to connect
    // Simulate connection for now
    QTimer::singleShot(500, this, [this]() {
        m_connectionState = Connected;
        emit connectionStateChanged(m_connectionState);

        // Mock channels
        m_channels.clear();
        m_channels.append({"channel-0", "Main", "Main communication channel"});
        m_channels.append({"channel-1", "Channel 1", ""});
        m_channels.append({"channel-2", "Channel 2", ""});
        emit channelsChanged();

        // Default talk channel
        m_talkChannel = "channel-0";
    });
}

void IntercomClient::disconnect()
{
    if (m_connectionState != Connected) {
        return;
    }

    if (m_isTalking) {
        stopTalk();
    }

    m_connectionState = Disconnecting;
    emit connectionStateChanged(m_connectionState);

    // TODO: Call Rust FFI to disconnect
    // Simulate disconnection for now
    QTimer::singleShot(100, this, [this]() {
        m_connectionState = Disconnected;
        m_roomId.clear();
        m_channels.clear();
        m_users.clear();
        emit connectionStateChanged(m_connectionState);
        emit channelsChanged();
        emit usersChanged();
    });
}

void IntercomClient::subscribeChannel(const QString &channelId)
{
    // TODO: Call Rust FFI
    qDebug() << "Subscribe to channel:" << channelId;
}

void IntercomClient::unsubscribeChannel(const QString &channelId)
{
    // TODO: Call Rust FFI
    qDebug() << "Unsubscribe from channel:" << channelId;
}

void IntercomClient::startTalk()
{
    if (!isConnected()) {
        emit errorOccurred("Not connected");
        return;
    }

    if (m_talkChannel.isEmpty()) {
        emit errorOccurred("No talk channel selected");
        return;
    }

    // TODO: Call Rust FFI to start talking
    m_isTalking = true;
    emit talkStateChanged(true);
    qDebug() << "Started talking on channel:" << m_talkChannel;
}

void IntercomClient::stopTalk()
{
    if (!m_isTalking) {
        return;
    }

    // TODO: Call Rust FFI to stop talking
    m_isTalking = false;
    emit talkStateChanged(false);
    qDebug() << "Stopped talking";
}

void IntercomClient::setTalkChannel(const QString &channelId)
{
    m_talkChannel = channelId;
}
