#include "ConnectDialog.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QFormLayout>
#include <QDialogButtonBox>
#include <QPushButton>

ConnectDialog::ConnectDialog(QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle("Connect to Room");
    setMinimumWidth(350);

    QVBoxLayout *mainLayout = new QVBoxLayout(this);

    // Form
    QFormLayout *formLayout = new QFormLayout();

    m_displayNameEdit = new QLineEdit(this);
    m_displayNameEdit->setPlaceholderText("Enter your name");
    formLayout->addRow("Display Name:", m_displayNameEdit);

    m_roomIdEdit = new QLineEdit(this);
    m_roomIdEdit->setPlaceholderText("Enter room ID");
    formLayout->addRow("Room ID:", m_roomIdEdit);

    mainLayout->addLayout(formLayout);
    mainLayout->addStretch();

    // Buttons
    QDialogButtonBox *buttonBox = new QDialogButtonBox(this);
    QPushButton *connectButton = buttonBox->addButton("Connect", QDialogButtonBox::AcceptRole);
    buttonBox->addButton(QDialogButtonBox::Cancel);

    connect(buttonBox, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttonBox, &QDialogButtonBox::rejected, this, &QDialog::reject);

    // Disable connect button until form is filled
    auto updateConnectButton = [this, connectButton]() {
        connectButton->setEnabled(
            !m_displayNameEdit->text().isEmpty() &&
            !m_roomIdEdit->text().isEmpty()
        );
    };

    connect(m_displayNameEdit, &QLineEdit::textChanged, updateConnectButton);
    connect(m_roomIdEdit, &QLineEdit::textChanged, updateConnectButton);
    updateConnectButton();

    mainLayout->addWidget(buttonBox);
}

QString ConnectDialog::roomId() const
{
    return m_roomIdEdit->text();
}

QString ConnectDialog::displayName() const
{
    return m_displayNameEdit->text();
}
