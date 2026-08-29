#ifndef DCTP_PORT_H
#define DCTP_PORT_H

#include <stdbool.h>
#include <stdint.h>

typedef struct {
    bool enabled;
    float base_pwm;
    float kp;
    float kd;
} dctp_tuning_t;

typedef struct {
    uint8_t line_bits;
    float line_error;
    int32_t encoder_left_count;
    int32_t encoder_right_count;
    int32_t encoder_left_cps;
    int32_t encoder_right_cps;
    float motor_left_pwm;
    float motor_right_pwm;
} dctp_telemetry_t;

bool dctp_port_init(void);
void dctp_port_poll(uint32_t now_ms);
dctp_tuning_t dctp_port_get_tuning(void);
void dctp_port_set_enabled(bool enabled);
void dctp_port_set_telemetry(const dctp_telemetry_t *telemetry);
void dctp_port_send_note(const char *text);

#endif

