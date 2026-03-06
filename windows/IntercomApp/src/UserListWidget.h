#pragma once

#include <QWidget>
#include <QListWidget>
#include <QLabel>

class UserListWidget : public QWidget
{
    Q_OBJECT

public:
    explicit UserListWidget(QWidget *parent = nullptr);

    void setUsers(const QVector<struct UserInfo> &users);
    void setUserTalking(const QString &userId, bool talking);

private:
    QListWidget *m_listWidget;
    QLabel *m_headerLabel;
    QLabel *m_countLabel;
};
