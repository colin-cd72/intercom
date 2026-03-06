#pragma once

#include <QWidget>
#include <QListWidget>
#include <QLabel>

class ChannelListWidget : public QWidget
{
    Q_OBJECT

public:
    explicit ChannelListWidget(QWidget *parent = nullptr);

    void setChannels(const QVector<struct ChannelInfo> &channels);
    QString selectedChannel() const;

signals:
    void channelSelected(const QString &channelId);

private:
    QListWidget *m_listWidget;
    QLabel *m_headerLabel;
};
