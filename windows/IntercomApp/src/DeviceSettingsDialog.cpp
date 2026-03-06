#include "DeviceSettingsDialog.h"
#include "IntercomClient.h"
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QLabel>
#include <QGroupBox>
#include <QPushButton>
#include <QDialogButtonBox>

DeviceSettingsDialog::DeviceSettingsDialog(QWidget *parent)
    : QDialog(parent)
{
    setWindowTitle("Audio Settings");
    setMinimumWidth(400);

    QVBoxLayout *mainLayout = new QVBoxLayout(this);

    // DANTE preference
    m_preferDanteCheckbox = new QCheckBox("Prefer DANTE devices", this);
    m_preferDanteCheckbox->setToolTip(
        "Automatically select DANTE Virtual Soundcard when available"
    );
    m_preferDanteCheckbox->setChecked(true);
    connect(m_preferDanteCheckbox, &QCheckBox::stateChanged,
            this, &DeviceSettingsDialog::onPreferDanteChanged);
    mainLayout->addWidget(m_preferDanteCheckbox);

    mainLayout->addSpacing(10);

    // Input device
    QGroupBox *inputGroup = new QGroupBox("Input Device", this);
    QVBoxLayout *inputLayout = new QVBoxLayout(inputGroup);
    m_inputDeviceCombo = new QComboBox(this);
    connect(m_inputDeviceCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &DeviceSettingsDialog::onInputDeviceChanged);
    inputLayout->addWidget(m_inputDeviceCombo);
    mainLayout->addWidget(inputGroup);

    // Output device
    QGroupBox *outputGroup = new QGroupBox("Output Device", this);
    QVBoxLayout *outputLayout = new QVBoxLayout(outputGroup);
    m_outputDeviceCombo = new QComboBox(this);
    connect(m_outputDeviceCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &DeviceSettingsDialog::onOutputDeviceChanged);
    outputLayout->addWidget(m_outputDeviceCombo);
    mainLayout->addWidget(outputGroup);

    mainLayout->addStretch();

    // Buttons
    QDialogButtonBox *buttonBox = new QDialogButtonBox(
        QDialogButtonBox::Ok | QDialogButtonBox::Cancel,
        this
    );
    connect(buttonBox, &QDialogButtonBox::accepted, this, &QDialog::accept);
    connect(buttonBox, &QDialogButtonBox::rejected, this, &QDialog::reject);
    mainLayout->addWidget(buttonBox);

    refreshDevices();
}

void DeviceSettingsDialog::refreshDevices()
{
    m_inputDeviceCombo->clear();
    m_outputDeviceCombo->clear();

    // TODO: Get devices from IntercomClient via FFI
    // Mock data for now
    QVector<AudioDeviceInfo> inputDevices = {
        {"Dante Virtual Soundcard", true, false},
        {"Microphone (Realtek Audio)", false, true},
        {"USB Microphone", false, false}
    };

    QVector<AudioDeviceInfo> outputDevices = {
        {"Dante Virtual Soundcard", true, false},
        {"Speakers (Realtek Audio)", false, true},
        {"Headphones", false, false}
    };

    int selectedInput = 0;
    int selectedOutput = 0;

    for (int i = 0; i < inputDevices.size(); ++i) {
        const auto &device = inputDevices[i];
        QString label = device.name;
        if (device.isDante) {
            label += " [DANTE]";
        }
        if (device.isDefault) {
            label += " (Default)";
        }
        m_inputDeviceCombo->addItem(label, device.name);

        if (m_preferDanteCheckbox->isChecked() && device.isDante) {
            selectedInput = i;
        }
    }

    for (int i = 0; i < outputDevices.size(); ++i) {
        const auto &device = outputDevices[i];
        QString label = device.name;
        if (device.isDante) {
            label += " [DANTE]";
        }
        if (device.isDefault) {
            label += " (Default)";
        }
        m_outputDeviceCombo->addItem(label, device.name);

        if (m_preferDanteCheckbox->isChecked() && device.isDante) {
            selectedOutput = i;
        }
    }

    m_inputDeviceCombo->setCurrentIndex(selectedInput);
    m_outputDeviceCombo->setCurrentIndex(selectedOutput);
}

void DeviceSettingsDialog::onPreferDanteChanged(int state)
{
    refreshDevices();
}

void DeviceSettingsDialog::onInputDeviceChanged(int index)
{
    QString deviceName = m_inputDeviceCombo->itemData(index).toString();
    // TODO: Call IntercomClient to set input device
    qDebug() << "Selected input device:" << deviceName;
}

void DeviceSettingsDialog::onOutputDeviceChanged(int index)
{
    QString deviceName = m_outputDeviceCombo->itemData(index).toString();
    // TODO: Call IntercomClient to set output device
    qDebug() << "Selected output device:" << deviceName;
}
