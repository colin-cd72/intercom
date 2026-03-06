#include "UserListWidget.h"
#include "IntercomClient.h"
#include <QVBoxLayout>
#include <QHBoxLayout>

UserListWidget::UserListWidget(QWidget *parent)
    : QWidget(parent)
{
    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);

    // Header with count
    QWidget *headerWidget = new QWidget(this);
    headerWidget->setStyleSheet("background-color: #2d2d2d;");
    QHBoxLayout *headerLayout = new QHBoxLayout(headerWidget);
    headerLayout->setContentsMargins(12, 8, 12, 8);

    m_headerLabel = new QLabel("Users", this);
    m_headerLabel->setStyleSheet("font-weight: bold;");
    headerLayout->addWidget(m_headerLabel);

    headerLayout->addStretch();

    m_countLabel = new QLabel("0", this);
    m_countLabel->setStyleSheet(
        "background-color: #404040; padding: 2px 8px; border-radius: 8px; font-size: 11px;"
    );
    headerLayout->addWidget(m_countLabel);

    layout->addWidget(headerWidget);

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
            background-color: #404040;
        }
    )");
    layout->addWidget(m_listWidget);
}

void UserListWidget::setUsers(const QVector<UserInfo> &users)
{
    m_listWidget->clear();
    for (const auto &user : users) {
        QString icon = user.isTalking ? "🎤 " : "👤 ";
        QListWidgetItem *item = new QListWidgetItem(icon + user.displayName);
        item->setData(Qt::UserRole, user.id);
        if (user.isTalking) {
            item->setForeground(QColor("#4CAF50"));
        }
        m_listWidget->addItem(item);
    }
    m_countLabel->setText(QString::number(users.size()));
}

void UserListWidget::setUserTalking(const QString &userId, bool talking)
{
    for (int i = 0; i < m_listWidget->count(); ++i) {
        QListWidgetItem *item = m_listWidget->item(i);
        if (item->data(Qt::UserRole).toString() == userId) {
            QString text = item->text();
            if (talking) {
                if (!text.startsWith("🎤")) {
                    text = "🎤 " + text.mid(3);
                }
                item->setForeground(QColor("#4CAF50"));
            } else {
                if (!text.startsWith("👤")) {
                    text = "👤 " + text.mid(3);
                }
                item->setForeground(QColor("#ffffff"));
            }
            item->setText(text);
            break;
        }
    }
}
