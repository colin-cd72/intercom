#pragma once

#include <QDialog>
#include <QLineEdit>

class ConnectDialog : public QDialog
{
    Q_OBJECT

public:
    explicit ConnectDialog(QWidget *parent = nullptr);

    QString roomId() const;
    QString displayName() const;

private:
    QLineEdit *m_displayNameEdit;
    QLineEdit *m_roomIdEdit;
};
