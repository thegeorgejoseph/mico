// These helpers keep presentation-only string shaping out of the feature components.
export function formatBytes(bytes: number) {
  if (!bytes) {
    return "Unknown";
  }
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value >= 10 || index === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[index]}`;
}

// Desktop settings and activity surfaces use the same lightweight relative-ish timestamp.
export function formatTimestamp(value: string) {
  if (!value) {
    return "Not checked yet";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "Unknown";
  }
  return date.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function capitalize(value: string) {
  if (!value) {
    return value;
  }
  return value[0].toUpperCase() + value.slice(1);
}
