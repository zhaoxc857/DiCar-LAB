#include "dctp_port.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "main.h"

#define RX_RING_SIZE 256u
#define RX_DRAIN_MAX 64u
#define RX_LINE_SIZE 192u
#define TX_BUFFER_SIZE 320u
#define CORE_TELEMETRY_PERIOD_MS 250u
#define DETAIL_TELEMETRY_PERIOD_MS 1000u

extern UART_HandleTypeDef huart1;

typedef struct {
    dctp_tuning_t tuning;
    dctp_telemetry_t telemetry;
    uint32_t core_tick;
    uint32_t detail_tick;
} dctp_context_t;

static dctp_context_t context;
static uint8_t rx_ring[RX_RING_SIZE];
static volatile uint8_t rx_head;
static volatile uint8_t rx_tail;
static uint8_t rx_byte;
static char rx_line[RX_LINE_SIZE];
static uint16_t rx_line_length;

static uint8_t tx_active[TX_BUFFER_SIZE];
static uint8_t tx_pending[TX_BUFFER_SIZE];
static volatile bool tx_busy;
static uint16_t tx_pending_length;
static bool tx_pending_priority;

static float clamp(float value, float minimum, float maximum)
{
    if (value < minimum) {
        return minimum;
    }
    if (value > maximum) {
        return maximum;
    }
    return value;
}

static const char *json_value(const char *line, const char *field)
{
    const char *position = strstr(line, field);

    if (position == NULL) {
        return NULL;
    }
    position = strchr(position + strlen(field), ':');
    if (position == NULL) {
        return NULL;
    }
    ++position;
    while ((*position == ' ') || (*position == '\t')) {
        ++position;
    }
    return position;
}

static bool json_string(const char *line, const char *field,
                        char *output, size_t output_size)
{
    const char *position = json_value(line, field);
    size_t length = 0u;

    if ((position == NULL) || (*position != '"') || (output_size == 0u)) {
        return false;
    }
    ++position;
    while ((position[length] != '\0') && (position[length] != '"')) {
        if ((position[length] == '\\') || ((length + 1u) >= output_size)) {
            return false;
        }
        output[length] = position[length];
        ++length;
    }
    if (position[length] != '"') {
        return false;
    }
    output[length] = '\0';
    return true;
}

static bool json_float(const char *line, const char *field, float *value)
{
    const char *position = json_value(line, field);
    char *end;

    if (position == NULL) {
        return false;
    }
    *value = strtof(position, &end);
    return end != position;
}

static bool json_u32(const char *line, const char *field, uint32_t *value)
{
    const char *position = json_value(line, field);
    char *end;
    unsigned long parsed;

    if (position == NULL) {
        return false;
    }
    parsed = strtoul(position, &end, 10);
    if (end == position) {
        return false;
    }
    *value = (uint32_t)parsed;
    return true;
}

static void tx_service(void)
{
    uint16_t length = 0u;
    uint32_t primask;

    primask = __get_PRIMASK();
    __disable_irq();
    if ((!tx_busy) && (tx_pending_length > 0u)) {
        length = tx_pending_length;
        memcpy(tx_active, tx_pending, length);
        tx_pending_length = 0u;
        tx_pending_priority = false;
        tx_busy = true;
    }
    if (primask == 0u) {
        __enable_irq();
    }

    if ((length > 0u) &&
        (HAL_UART_Transmit_IT(&huart1, tx_active, length) != HAL_OK)) {
        tx_busy = false;
    }
}

static void queue_tx(const char *text, bool priority)
{
    const size_t length = strlen(text);
    uint32_t primask;

    if ((length == 0u) || (length >= TX_BUFFER_SIZE)) {
        return;
    }

    primask = __get_PRIMASK();
    __disable_irq();
    if ((tx_pending_length == 0u) ||
        (priority && !tx_pending_priority)) {
        memcpy(tx_pending, text, length);
        tx_pending_length = (uint16_t)length;
        tx_pending_priority = priority;
    }
    if (primask == 0u) {
        __enable_irq();
    }
    tx_service();
}

static bool get_parameter(const char *key, float *value)
{
    if (strcmp(key, "control_enabled") == 0) {
        *value = context.tuning.enabled ? 1.0f : 0.0f;
    } else if (strcmp(key, "base_pwm") == 0) {
        *value = context.tuning.base_pwm;
    } else if (strcmp(key, "line_kp") == 0) {
        *value = context.tuning.kp;
    } else if (strcmp(key, "line_kd") == 0) {
        *value = context.tuning.kd;
    } else {
        return false;
    }
    return true;
}

static bool set_parameter(const char *key, float value, float *actual)
{
    if (strcmp(key, "control_enabled") == 0) {
        context.tuning.enabled = value >= 0.5f;
    } else if (strcmp(key, "base_pwm") == 0) {
        context.tuning.base_pwm = clamp(value, 0.0f, 60.0f);
    } else if (strcmp(key, "line_kp") == 0) {
        context.tuning.kp = clamp(value, 0.0f, 20.0f);
    } else if (strcmp(key, "line_kd") == 0) {
        context.tuning.kd = clamp(value, 0.0f, 20.0f);
    } else {
        return false;
    }
    return get_parameter(key, actual);
}

static void send_ack(const char *key, float value, uint32_t sequence,
                     bool has_sequence, bool ok)
{
    char line[TX_BUFFER_SIZE];
    int length;

    if (has_sequence) {
        length = snprintf(line, sizeof line,
                          "{\"type\":\"ACK\",\"key\":\"%s\","
                          "\"value\":%.3f,\"seq\":%lu,\"ok\":%s%s}\n",
                          key, (double)value, (unsigned long)sequence,
                          ok ? "true" : "false",
                          ok ? "" : ",\"error\":\"unknown_key\"");
    } else {
        length = snprintf(line, sizeof line,
                          "{\"type\":\"ACK\",\"key\":\"%s\","
                          "\"value\":%.3f,\"ok\":%s%s}\n",
                          key, (double)value, ok ? "true" : "false",
                          ok ? "" : ",\"error\":\"unknown_command\"");
    }
    if ((length > 0) && ((size_t)length < sizeof line)) {
        queue_tx(line, true);
    }
}

static void process_line(const char *line)
{
    char type[8];
    char key[32];
    float value = 0.0f;
    uint32_t sequence = 0u;
    bool has_sequence;
    bool ok;

    if (!json_string(line, "\"type\"", type, sizeof type) ||
        !json_string(line, "\"key\"", key, sizeof key)) {
        return;
    }
    has_sequence = json_u32(line, "\"seq\"", &sequence);

    if (strcmp(type, "GET") == 0) {
        ok = get_parameter(key, &value);
        send_ack(key, value, sequence, has_sequence, ok);
    } else if (strcmp(type, "SET") == 0) {
        ok = json_float(line, "\"value\"", &value) &&
             set_parameter(key, value, &value);
        send_ack(key, value, sequence, has_sequence, ok);
    } else if (strcmp(type, "CMD") == 0) {
        if (strcmp(key, "emergency_stop") == 0) {
            context.tuning.enabled = false;
            ok = true;
        } else if (strcmp(key, "fw_version") == 0) {
            char note[64];
            (void)snprintf(note, sizeof note,
                           "{\"type\":\"NOTE\",\"data\":\"fw_version="
                           DCTP_PORT_VERSION "\"}\n");
            queue_tx(note, true);
            ok = true;
        } else {
            ok = false;
        }
        send_ack(key, 0.0f, sequence, has_sequence, ok);
    }
}

static void feed_rx_byte(uint8_t byte)
{
    if (byte == (uint8_t)'\n') {
        if (rx_line_length > 0u) {
            if ((rx_line_length > 0u) &&
                (rx_line[rx_line_length - 1u] == '\r')) {
                --rx_line_length;
            }
            rx_line[rx_line_length] = '\0';
            process_line(rx_line);
        }
        rx_line_length = 0u;
    } else if (rx_line_length < (RX_LINE_SIZE - 1u)) {
        rx_line[rx_line_length++] = (char)byte;
    } else {
        rx_line_length = 0u;
    }
}

static void send_core_telemetry(void)
{
    char line[TX_BUFFER_SIZE];
    const dctp_telemetry_t *t = &context.telemetry;
    const int length = snprintf(
        line, sizeof line,
        "{\"type\":\"TEL\",\"data\":{"
        "\"line_error\":%.3f,\"line_bits\":%u,"
        "\"left_cps\":%ld,\"right_cps\":%ld,"
        "\"left_pwm\":%.2f,\"right_pwm\":%.2f}}\n",
        (double)t->line_error, (unsigned int)t->line_bits,
        (long)t->encoder_left_cps, (long)t->encoder_right_cps,
        (double)t->motor_left_pwm, (double)t->motor_right_pwm);

    if ((length > 0) && ((size_t)length < sizeof line)) {
        queue_tx(line, false);
    }
}

static void send_detail_telemetry(void)
{
    char line[TX_BUFFER_SIZE];
    const dctp_telemetry_t *t = &context.telemetry;
    const int length = snprintf(
        line, sizeof line,
        "{\"type\":\"TEL\",\"data\":{"
        "\"line_0\":%u,\"line_1\":%u,\"line_2\":%u,\"line_3\":%u,"
        "\"line_4\":%u,\"line_5\":%u,\"line_6\":%u,\"line_7\":%u,"
        "\"left_count\":%ld,\"right_count\":%ld}}\n",
        (unsigned int)((t->line_bits >> 0u) & 1u),
        (unsigned int)((t->line_bits >> 1u) & 1u),
        (unsigned int)((t->line_bits >> 2u) & 1u),
        (unsigned int)((t->line_bits >> 3u) & 1u),
        (unsigned int)((t->line_bits >> 4u) & 1u),
        (unsigned int)((t->line_bits >> 5u) & 1u),
        (unsigned int)((t->line_bits >> 6u) & 1u),
        (unsigned int)((t->line_bits >> 7u) & 1u),
        (long)t->encoder_left_count, (long)t->encoder_right_count);

    if ((length > 0) && ((size_t)length < sizeof line)) {
        queue_tx(line, false);
    }
}

bool dctp_port_init(void)
{
    memset(&context, 0, sizeof context);
    context.tuning.base_pwm = 20.0f;
    context.tuning.kp = 4.0f;
    context.tuning.kd = 2.0f;
    rx_head = 0u;
    rx_tail = 0u;
    rx_line_length = 0u;
    tx_busy = false;
    tx_pending_length = 0u;
    tx_pending_priority = false;
    return HAL_UART_Receive_IT(&huart1, &rx_byte, 1u) == HAL_OK;
}

void dctp_port_poll(uint32_t now_ms)
{
    uint8_t count = 0u;

    while ((rx_tail != rx_head) && (count < RX_DRAIN_MAX)) {
        feed_rx_byte(rx_ring[rx_tail++]);
        ++count;
    }

    tx_service();
    if ((uint32_t)(now_ms - context.detail_tick) >=
        DETAIL_TELEMETRY_PERIOD_MS) {
        context.detail_tick = now_ms;
        send_detail_telemetry();
    } else if ((uint32_t)(now_ms - context.core_tick) >=
               CORE_TELEMETRY_PERIOD_MS) {
        context.core_tick = now_ms;
        send_core_telemetry();
    }
}

dctp_tuning_t dctp_port_get_tuning(void)
{
    return context.tuning;
}

void dctp_port_set_enabled(bool enabled)
{
    context.tuning.enabled = enabled;
}

void dctp_port_set_telemetry(const dctp_telemetry_t *telemetry)
{
    if (telemetry != NULL) {
        context.telemetry = *telemetry;
    }
}

void dctp_port_send_note(const char *text)
{
    char line[TX_BUFFER_SIZE];
    size_t out = 0u;
    size_t in = 0u;

    if (text == NULL) {
        return;
    }
    line[0] = '\0';
    out = (size_t)snprintf(line, sizeof line,
                           "{\"type\":\"NOTE\",\"data\":\"");
    while ((text[in] != '\0') &&
           ((out + 6u) < ((size_t)sizeof line - 3u))) {
        const char c = text[in];

        if ((c == '"') || (c == '\\')) {
            line[out++] = ' ';
        } else if ((c >= 0x20u) && (c < 0x7fu)) {
            line[out++] = c;
        }
        ++in;
    }
    line[out++] = '"';
    line[out++] = '}';
    line[out++] = '\n';
    line[out] = '\0';
    queue_tx(line, true);
}

void HAL_UART_RxCpltCallback(UART_HandleTypeDef *huart)
{
    if (huart == &huart1) {
        const uint8_t next = (uint8_t)(rx_head + 1u);
        if (next != rx_tail) {
            rx_ring[rx_head] = rx_byte;
            rx_head = next;
        }
        (void)HAL_UART_Receive_IT(&huart1, &rx_byte, 1u);
    }
}

void HAL_UART_TxCpltCallback(UART_HandleTypeDef *huart)
{
    if (huart == &huart1) {
        tx_busy = false;
    }
}
