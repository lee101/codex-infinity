#pragma once

#include <linux/capability.h>

typedef int cap_value_t;

int cap_from_name(const char *name, cap_value_t *cap_value);
int capget(cap_user_header_t header, const cap_user_data_t data);
int capset(cap_user_header_t header, const cap_user_data_t data);
