#pragma once

#include <QWidget>
#include <QPushButton>
#include <QLabel>
#include <QProgressBar>

class TalkButtonWidget : public QWidget
{
    Q_OBJECT

public:
    explicit TalkButtonWidget(QWidget *parent = nullptr);

    void setEnabled(bool enabled);
    void setTalking(bool talking);
    void setPressed(bool pressed);
    void setAudioLevel(float level);

signals:
    void talkStarted();
    void talkStopped();

protected:
    void paintEvent(QPaintEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;

private:
    void updateButtonAppearance();

    QPushButton *m_button;
    QLabel *m_statusLabel;
    QLabel *m_instructionLabel;
    QProgressBar *m_levelMeter;

    bool m_enabled = true;
    bool m_talking = false;
    bool m_pressed = false;
    float m_audioLevel = 0.0f;
};
