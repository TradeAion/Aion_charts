/** Shared feature fixtures consumed through each library's public API. */
export function marker_fixture(data) {
  const marker = (index, position, shape, color, text) => ({
    time: data[index].time,
    position,
    shape,
    color,
    text,
  });
  return [
    marker(840, "aboveBar", "arrowDown", "#ef5350", "SELL"),
    marker(880, "belowBar", "arrowUp", "#26a69a", "BUY"),
    marker(920, "inBar", "circle", "#7e57c2", "MID"),
    marker(960, "aboveBar", "square", "#2962ff", "NOTE"),
  ];
}

export function volume_fixture(data) {
  return data.map((bar) => ({
    time: bar.time,
    value: Math.round(500 + Math.abs(bar.close - bar.open) * 4000 + 300),
    color: bar.close >= bar.open ? "#26a69a80" : "#ef535080",
  }));
}
