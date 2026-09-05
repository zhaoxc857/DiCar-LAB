from PySide6.QtWidgets import QWidget, QGridLayout

from core.fw_version import FwVersionProbe
from ui.widgets import ValueCard


class OverviewPage(QWidget):
    def __init__(self, bus, config, transport=None):
        super().__init__()
        grid = QGridLayout(self)
        self.cards = {
            "speed": ValueCard("车速", "m/s"),
            "actual_rpm": ValueCard("实际 RPM", "rpm"),
            "yaw": ValueCard("航向角", "°"),
            "gyro_z": ValueCard("横摆角速度", "°/s"),
            "battery": ValueCard("电池电压", "V"),
            "steering_output": ValueCard("转向输出", "%"),
            "@fw_version": ValueCard("车上固件版本", ""),
        }
        for i, c in enumerate(self.cards.values()): grid.addWidget(c, i//3, i%3)
        bus.telemetry.connect(self._tel)
        self._probe = FwVersionProbe(bus, transport, self)
        self._probe.version.connect(lambda v: self.cards["@fw_version"].set_value(v))
    def _tel(self, d):
        for k, c in self.cards.items():
            if k == "@fw_version":
                continue
            if k in d: c.set_value(d[k])
