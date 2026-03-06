#include "ChannelListWidget.h"
#include "IntercomClient.h"
#include <QVBoxLayout>

ChannelListWidget::ChannelListWidget(QWidget *parent)
    : QWidget(parent)
{
    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    // Header
    m_headerLabel = new QLabel("Channels", this);
    m_headerLabel->setStyleSheet(
        "background-color: #2d2d2d; padding: 8px 12px; font-weight: bold;"
    );
    layout->addWidget(m_headerLabel);

    // List
    m_listWidget = new QListWidget(this);
    m_listWidget->setStyleSheet(R"(
        QListWidget {
            background-color: #353535;
            border: none;
            outline: none;
        }
        QListWidget::item {
            padding: 8px 12px;
            border-bottom: 1px solid #404040;
        }
        QListWidget::item:selected {
            background-color: #4CAF50;
        }
        QListWidget::item:hover:!selected {
            background-color: #404040;
        }
    )");
    layout->addWidget(m_listWidget);

    connect(m_listWidget, &QListWidget::currentRowChanged, this, [this](int row) {
        if (row >= 0) {
            QListWidgetItem *item = m_listWidget->item(row);
            emit channelSelected(item->data(Qt::UserRole).toString());
        }
    });
}

void ChannelListWidget::setChannels(const QVector<ChannelInfo> &channels)
{
    m_listWidget->clear();
    for (const auto &channel : channels) {
        QListWidgetItem *item = new QListWidgetItem(channel.name);
        item->setData(Qt::UserRole, channel.id);
        if (!channel.description.isEmpty()) {
            item->setToolTip(channel.description);
        }
        m_listWidget->addItem(item);
    }

    // Select first channel by default
    if (m_listWidget->count() > 0) {
        m_listWidget->setCurrentRow(0);
    }
}

QString ChannelListWidget::selectedChannel() const
{
    QListWidgetItem *item = m_listWidget->currentItem();
    return item ? item->data(Qt::UserRole).toString() : QString();
}
