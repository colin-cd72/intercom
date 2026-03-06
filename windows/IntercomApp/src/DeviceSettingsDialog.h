#pragma once

#include <QDialog>
#include <QComboBox>
#include <QCheckBox>

class DeviceSettingsDialog : public QDialog
{
    Q_OBJECT

public:
    explicit DeviceSettingsDialog(QWidget *parent = nullptr);

private slots:
    void onPreferDanteChanged(int state);
    void onInputDeviceChanged(int index);
    void onOutputDeviceChanged(int index);

private:
    void refreshDevices();

    QCheckBox *m_preferDanteCheckbox;
    QComboBox *m_inputDeviceCombo;
    QComboBox *m_outputDeviceCombo;
};
