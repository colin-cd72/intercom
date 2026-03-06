#include "TalkButtonWidget.h"
#include <QVBoxLayout>
#include <QPainter>
#include <QMouseEvent>

TalkButtonWidget::TalkButtonWidget(QWidget *parent)
    : QWidget(parent)
{
    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->setAlignment(Qt::AlignCenter);
    layout->setSpacing(16);

    // Status label
    m_statusLabel = new QLabel("Ready", this);
    m_statusLabel->setAlignment(Qt::AlignCenter);
    m_statusLabel->setStyleSheet("color: #888; font-size: 12px;");
    layout->addWidget(m_statusLabel);

    // Talk button
    m_button = new QPushButton(this);
    m_button->setFixedSize(120, 120);
    m_button->setCursor(Qt::PointingHandCursor);
    m_button->setFocusPolicy(Qt::NoFocus);
    layout->addWidget(m_button, 0, Qt::AlignCenter);

    // Connect button events
    connect(m_button, &QPushButton::pressed, this, [this]() {
        if (m_enabled) {
            setPressed(true);
        }
    });
    connect(m_button, &QPushButton::released, this, [this]() {
        if (m_enabled) {
            setPressed(false);
        }
    });

    // Audio level meter
    m_levelMeter = new QProgressBar(this);
    m_levelMeter->setFixedSize(120, 8);
    m_levelMeter->setRange(0, 100);
    m_levelMeter->setValue(0);
    m_levelMeter->setTextVisible(false);
    m_levelMeter->setStyleSheet(
        "QProgressBar { background-color: #333; border: none; border-radius: 4px; }"
        "QProgressBar::chunk { background-color: #4CAF50; border-radius: 4px; }"
    );
    m_levelMeter->setVisible(false);
    layout->addWidget(m_levelMeter, 0, Qt::AlignCenter);

    // Instruction label
    m_instructionLabel = new QLabel("Hold to talk • Space bar", this);
    m_instructionLabel->setAlignment(Qt::AlignCenter);
    m_instructionLabel->setStyleSheet("color: #666; font-size: 10px;");
    layout->addWidget(m_instructionLabel);

    updateButtonAppearance();
}

void TalkButtonWidget::setEnabled(bool enabled)
{
    m_enabled = enabled;
    m_button->setEnabled(enabled);
    updateButtonAppearance();
}

void TalkButtonWidget::setTalking(bool talking)
{
    m_talking = talking;
    m_levelMeter->setVisible(talking);
    updateButtonAppearance();
}

void TalkButtonWidget::setPressed(bool pressed)
{
    if (m_pressed == pressed) return;

    m_pressed = pressed;
    if (pressed) {
        emit talkStarted();
    } else {
        emit talkStopped();
    }
    updateButtonAppearance();
}

void TalkButtonWidget::setAudioLevel(float level)
{
    m_audioLevel = level;
    m_levelMeter->setValue(static_cast<int>(level * 100));
}

void TalkButtonWidget::paintEvent(QPaintEvent *event)
{
    QWidget::paintEvent(event);
}

void TalkButtonWidget::mousePressEvent(QMouseEvent *event)
{
    QWidget::mousePressEvent(event);
}

void TalkButtonWidget::mouseReleaseEvent(QMouseEvent *event)
{
    QWidget::mouseReleaseEvent(event);
}

void TalkButtonWidget::updateButtonAppearance()
{
    QString baseStyle = R"(
        QPushButton {
            border-radius: 60px;
            font-size: 14px;
            font-weight: bold;
            color: white;
        }
    )";

    QString bgColor, hoverColor, pressedColor, shadowColor;

    if (!m_enabled) {
        bgColor = "#666";
        hoverColor = "#666";
        pressedColor = "#666";
        m_button->setText("TALK");
        m_statusLabel->setText("Not Connected");
        m_statusLabel->setStyleSheet("color: #888; font-size: 12px;");
    } else if (m_talking) {
        bgColor = "#f44336";
        hoverColor = "#e53935";
        pressedColor = "#d32f2f";
        m_button->setText("TALKING");
        m_statusLabel->setText("Transmitting...");
        m_statusLabel->setStyleSheet("color: #4CAF50; font-size: 12px;");
    } else {
        bgColor = "#4CAF50";
        hoverColor = "#43A047";
        pressedColor = "#388E3C";
        m_button->setText("TALK");
        m_statusLabel->setText("Ready");
        m_statusLabel->setStyleSheet("color: #888; font-size: 12px;");
    }

    QString style = baseStyle + QString(R"(
        QPushButton {
            background-color: %1;
        }
        QPushButton:hover {
            background-color: %2;
        }
        QPushButton:pressed {
            background-color: %3;
        }
        QPushButton:disabled {
            background-color: #555;
            color: #999;
        }
    )").arg(bgColor, hoverColor, pressedColor);

    m_button->setStyleSheet(style);
}
