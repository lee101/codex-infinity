#include "capability_compat.h"

#include <stddef.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

struct capability_name {
  const char *name;
  cap_value_t value;
};

static const struct capability_name CAPABILITY_NAMES[] = {
    {"CAP_CHOWN", CAP_CHOWN},
    {"CAP_DAC_OVERRIDE", CAP_DAC_OVERRIDE},
    {"CAP_DAC_READ_SEARCH", CAP_DAC_READ_SEARCH},
    {"CAP_FOWNER", CAP_FOWNER},
    {"CAP_FSETID", CAP_FSETID},
    {"CAP_KILL", CAP_KILL},
    {"CAP_SETGID", CAP_SETGID},
    {"CAP_SETUID", CAP_SETUID},
    {"CAP_SETPCAP", CAP_SETPCAP},
    {"CAP_LINUX_IMMUTABLE", CAP_LINUX_IMMUTABLE},
    {"CAP_NET_BIND_SERVICE", CAP_NET_BIND_SERVICE},
    {"CAP_NET_BROADCAST", CAP_NET_BROADCAST},
    {"CAP_NET_ADMIN", CAP_NET_ADMIN},
    {"CAP_NET_RAW", CAP_NET_RAW},
    {"CAP_IPC_LOCK", CAP_IPC_LOCK},
    {"CAP_IPC_OWNER", CAP_IPC_OWNER},
    {"CAP_SYS_MODULE", CAP_SYS_MODULE},
    {"CAP_SYS_RAWIO", CAP_SYS_RAWIO},
    {"CAP_SYS_CHROOT", CAP_SYS_CHROOT},
    {"CAP_SYS_PTRACE", CAP_SYS_PTRACE},
    {"CAP_SYS_PACCT", CAP_SYS_PACCT},
    {"CAP_SYS_ADMIN", CAP_SYS_ADMIN},
    {"CAP_SYS_BOOT", CAP_SYS_BOOT},
    {"CAP_SYS_NICE", CAP_SYS_NICE},
    {"CAP_SYS_RESOURCE", CAP_SYS_RESOURCE},
    {"CAP_SYS_TIME", CAP_SYS_TIME},
    {"CAP_SYS_TTY_CONFIG", CAP_SYS_TTY_CONFIG},
    {"CAP_MKNOD", CAP_MKNOD},
    {"CAP_LEASE", CAP_LEASE},
    {"CAP_AUDIT_WRITE", CAP_AUDIT_WRITE},
    {"CAP_AUDIT_CONTROL", CAP_AUDIT_CONTROL},
    {"CAP_SETFCAP", CAP_SETFCAP},
    {"CAP_MAC_OVERRIDE", CAP_MAC_OVERRIDE},
    {"CAP_MAC_ADMIN", CAP_MAC_ADMIN},
    {"CAP_SYSLOG", CAP_SYSLOG},
    {"CAP_WAKE_ALARM", CAP_WAKE_ALARM},
    {"CAP_BLOCK_SUSPEND", CAP_BLOCK_SUSPEND},
    {"CAP_AUDIT_READ", CAP_AUDIT_READ},
    {"CAP_PERFMON", CAP_PERFMON},
    {"CAP_BPF", CAP_BPF},
    {"CAP_CHECKPOINT_RESTORE", CAP_CHECKPOINT_RESTORE},
};

int cap_from_name(const char *name, cap_value_t *cap_value) {
  if (name == NULL || cap_value == NULL) {
    return -1;
  }

  for (size_t i = 0; i < sizeof(CAPABILITY_NAMES) / sizeof(CAPABILITY_NAMES[0]); i++) {
    if (strcasecmp(name, CAPABILITY_NAMES[i].name) == 0) {
      *cap_value = CAPABILITY_NAMES[i].value;
      return 0;
    }
  }

  return -1;
}

int capget(cap_user_header_t header, const cap_user_data_t data) {
  return syscall(SYS_capget, header, data);
}

int capset(cap_user_header_t header, const cap_user_data_t data) {
  return syscall(SYS_capset, header, data);
}
