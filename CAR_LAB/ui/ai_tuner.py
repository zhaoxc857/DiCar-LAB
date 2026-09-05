from __future__ import annotations
import time
from collections import deque
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import (QWidget,QVBoxLayout,QHBoxLayout,QGridLayout,QGroupBox,QLabel,QPushButton,QDoubleSpinBox,QPlainTextEdit,QCheckBox,QMessageBox,QComboBox,QLineEdit,QTableWidget,QTableWidgetItem)
from core.metrics import response_metrics, score_speed_metrics
from core.history_store import HistoryStore
from core.sweep import parse_candidates, pick_best


class AITunerPage(QWidget):
    """Local explainable tuning helper. It never calls a cloud model."""
    def __init__(self,bus,transport,config):
        super().__init__(); self.bus=bus;self.transport=transport;self.config=config;self.cfg=config.get("speed_lab",{});self.samples=deque(maxlen=3000);self.running=False;self.started=0;self.store=HistoryStore()
        root=QVBoxLayout(self)
        box=QGroupBox("速度环阶跃分析 / 本地 AI 建议");g=QGridLayout(box)
        self.target=QDoubleSpinBox();self.target.setRange(-10000,10000);self.target.setValue(500);self.seconds=QDoubleSpinBox();self.seconds.setRange(.5,15);self.seconds.setValue(3.0);self.unlock=QCheckBox("我已确认实车已架空/安全，允许发送阶跃目标")
        start=QPushButton("开始一次阶跃分析"); start.setObjectName("primary"); start.clicked.connect(self._start);stop=QPushButton("立即停止");stop.clicked.connect(self._stop)
        g.addWidget(QLabel("目标 RPM"),0,0);g.addWidget(self.target,0,1);g.addWidget(QLabel("采集(s)"),0,2);g.addWidget(self.seconds,0,3);g.addWidget(self.unlock,1,0,1,4);g.addWidget(start,2,0,1,2);g.addWidget(stop,2,2,1,2);root.addWidget(box)
        root.addWidget(self._build_sweep_box())
        self.out=QPlainTextEdit();self.out.setReadOnly=True;root.addWidget(self.out,1)
        bus.telemetry.connect(self._tel);self.timer=QTimer(self);self.timer.timeout.connect(self._tick);self.timer.start(50)
        # 扫参状态机
        self.sweep_phase="idle";self.sweep_candidates=[];self.sweep_idx=0;self.sweep_samples=[];self.sweep_results=[];self.sweep_param_key="";self.sweep_original=None;self.sweep_phase_until=0.0;self.sweep_started=0.0
        self.sweep_timer=QTimer(self);self.sweep_timer.setInterval(50);self.sweep_timer.timeout.connect(self._sweep_tick)
    def _start(self):
        if self.transport.kind!="sim" and not self.unlock.isChecked():
            QMessageBox.warning(self,"安全确认","实车阶跃会让电机动作。请先架空车辆并勾选安全确认。") ;return
        self.samples.clear();self.running=True;self.started=time.monotonic();self.transport.command(self.cfg.get("target_command_key","target_rpm"),self.target.value());self.out.setPlainText("采集中...\n当前功能是本地规则/指标分析，不连接云端 AI。")
    def _stop(self):
        self.transport.command(self.cfg.get("target_command_key","target_rpm"),0.0);self.running=False
    def _build_sweep_box(self):
        box=QGroupBox("自动扫参（逐档下发参数并实测阶跃，自动评分排序）")
        layout=QVBoxLayout(box)
        row=QHBoxLayout()
        row.addWidget(QLabel("参数"))
        self.sweep_param=QComboBox()
        for label,key in (self.cfg.get("params",{}) or {}).items():
            self.sweep_param.addItem(str(label),str(key))
        row.addWidget(self.sweep_param)
        row.addWidget(QLabel("候选值"))
        self.sweep_values=QLineEdit("0.6,0.8,1.0,1.2")
        self.sweep_values.setToolTip("逗号分隔，按从小到大自动排序")
        row.addWidget(self.sweep_values,1)
        self.sweep_btn=QPushButton("开始扫参"); self.sweep_btn.setObjectName("primary"); self.sweep_btn.clicked.connect(self._sweep_start)
        self.sweep_stop=QPushButton("停止");self.sweep_stop.clicked.connect(self._sweep_stop)
        self.sweep_apply=QPushButton("应用最优参数");self.sweep_apply.setEnabled(False);self.sweep_apply.clicked.connect(self._sweep_apply_best)
        row.addWidget(self.sweep_btn);row.addWidget(self.sweep_stop);row.addWidget(self.sweep_apply)
        layout.addLayout(row)
        self.sweep_table=QTableWidget(0,6)
        self.sweep_table.setHorizontalHeaderLabels(["参数值","超调","上升时间","整定时间","稳态误差","评分"])
        self.sweep_table.setMaximumHeight(140)
        self.sweep_table.horizontalHeader().setStretchLastSection(True)
        layout.addWidget(self.sweep_table)
        return box

    SWEEP_SETTLE_S=0.8

    def _sweep_start(self):
        if self.transport.kind!="sim" and not self.unlock.isChecked():
            QMessageBox.warning(self,"安全确认","扫参会连续发送阶跃目标让电机反复动作。请先架空车辆并勾选安全确认。");return
        key=str(self.sweep_param.currentData() or "")
        if not key:
            QMessageBox.warning(self,"自动扫参","车型 YAML 的 speed_lab.params 未配置参数映射。");return
        try:
            candidates=parse_candidates(self.sweep_values.text())
        except ValueError:
            QMessageBox.warning(self,"自动扫参","候选值格式不对，示例：0.6, 0.8, 1.0, 1.2");return
        if len(candidates)<2:
            QMessageBox.warning(self,"自动扫参","至少需要两个候选值才能对比。");return
        self.sweep_param_key=key
        self.sweep_original=self.transport.param_cache.get(key)
        self.sweep_candidates=candidates;self.sweep_idx=0;self.sweep_results=[]
        self.sweep_table.setRowCount(0)
        self.out.setPlainText(f"扫参开始：{key} ∈ {candidates}，每档采集 {self.seconds.value():.1f}s…")
        self.sweep_btn.setEnabled(False);self.sweep_apply.setEnabled(False)
        self.sweep_timer.start()
        self._sweep_next_candidate()

    def _sweep_next_candidate(self):
        value=self.sweep_candidates[self.sweep_idx]
        self.transport.set_param(self.sweep_param_key,value)
        self.sweep_samples=[]
        self.sweep_phase="settle"
        self.sweep_phase_until=time.monotonic()+self.SWEEP_SETTLE_S
        self.sweep_started=time.monotonic()

    def _sweep_tick(self):
        now=time.monotonic()
        if self.sweep_phase=="pause" and now>=self.sweep_phase_until:
            self._sweep_next_candidate()
        elif self.sweep_phase=="settle" and now>=self.sweep_phase_until:
            self.sweep_phase="collect"
            self.sweep_started=now
            self.sweep_samples=[]
            self.transport.command(self.cfg.get("target_command_key","target_rpm"),self.target.value())
        elif self.sweep_phase=="collect" and now>=self.sweep_started+self.seconds.value():
            self.transport.command(self.cfg.get("target_command_key","target_rpm"),0.0)
            self._sweep_eval()

    def _sweep_eval(self):
        base=self.sweep_samples[0]["t"] if self.sweep_samples else 0.0
        samples=[{"t":s["t"]-base,"target":s["target"],"actual":s["actual"]} for s in self.sweep_samples]
        m=response_metrics(samples)
        score=score_speed_metrics(m)
        value=self.sweep_candidates[self.sweep_idx]
        result={"value":value,"score":score,**m}
        self.sweep_results.append(result)
        r=self.sweep_table.rowCount();self.sweep_table.insertRow(r)
        cells=[f"{value:g}",f"{float(m.get('overshoot_pct',0)):.1f}%",
               ("--" if m.get("rise_time_s") is None else f"{m['rise_time_s']:.2f}s"),
               ("--" if m.get("settling_time_s") is None else f"{m['settling_time_s']:.2f}s"),
               f"{float(m.get('steady_error',0)):.2f}",f"{score:.1f}"]
        for c,text in enumerate(cells):self.sweep_table.setItem(r,c,QTableWidgetItem(text))
        self.sweep_idx+=1
        if self.sweep_idx<len(self.sweep_candidates):
            self.sweep_phase="pause";self.sweep_phase_until=time.monotonic()+self.SWEEP_SETTLE_S
        else:
            self._sweep_finish()

    def _sweep_finish(self):
        self.sweep_phase="idle";self.sweep_timer.stop()
        if self.sweep_original is not None:
            self.transport.set_param(self.sweep_param_key,self.sweep_original)
        best=pick_best(self.sweep_results)
        self.sweep_btn.setEnabled(True);self.sweep_apply.setEnabled(best is not None)
        lines=[f"扫参完成（{self.sweep_param_key}），原始值 {self.sweep_original} 已恢复。"]
        if best is not None:
            lines.append(f"最优：{self.sweep_param_key}={best['value']:g}（评分 {float(best['score']):.1f}，超调 {float(best.get('overshoot_pct',0)):.1f}%）")
        self.out.setPlainText("\n".join(lines))
        self.store.add("speed_sweep","速度环扫参",self.config.get("vehicle",{}).get("display_name",""),
                       parameters={self.sweep_param_key:list(self.sweep_candidates)},
                       metrics={"best":best or {}})

    def _sweep_stop(self):
        if self.sweep_phase=="idle":return
        self.transport.command(self.cfg.get("target_command_key","target_rpm"),0.0)
        if self.sweep_original is not None:
            self.transport.set_param(self.sweep_param_key,self.sweep_original)
        self.sweep_phase="idle";self.sweep_timer.stop()
        self.sweep_btn.setEnabled(True)
        self.out.appendPlainText("扫参已中止，目标已归零、原始参数已恢复。")

    def _sweep_apply_best(self):
        best=pick_best(self.sweep_results)
        if best is None:return
        self.transport.set_param(self.sweep_param_key,best["value"])
        self.out.appendPlainText(f"已下发最优参数 {self.sweep_param_key}={best['value']:g}（等待 ACK 回读确认）。")

    def _tel(self,d):
        now=time.monotonic()
        if self.sweep_phase=="collect":
            tk=self.cfg.get("target_key","target_rpm");ak=self.cfg.get("actual_key","actual_rpm")
            self.sweep_samples.append({"t":now,"target":float(d.get(tk,self.target.value())),"actual":float(d.get(ak,0))})
        if not self.running:return
        now=time.monotonic()-self.started;tk=self.cfg.get("target_key","target_rpm");ak=self.cfg.get("actual_key","actual_rpm")
        self.samples.append({"t":now,"target":float(d.get(tk,self.target.value())),"actual":float(d.get(ak,0)),"output":float(d.get(self.cfg.get("output_key","motor_pwm"),0))})
    def _tick(self):
        if not self.running or time.monotonic()-self.started<self.seconds.value():return
        self.running=False; self.transport.command(self.cfg.get("target_command_key","target_rpm"),0.0)
        sm=list(self.samples);m=response_metrics(sm);score=score_speed_metrics(m);p=dict(self.transport.param_cache)
        suggestions=[]
        ov=float(m.get("overshoot_pct",0));rise=m.get("rise_time_s");steady=float(m.get("steady_error",0));cross=int(m.get("target_crossings",0))
        if rise is not None and rise>1.0 and ov<8:suggestions.append("响应偏慢且超调不大：可以小幅增加 Kp（例如 +5%~10%）后复测。")
        if ov>15 or cross>=3:suggestions.append("超调/反复过零较明显：优先降低 Kp；若已有 D 项，可小幅增加 Kd 并注意噪声。")
        if steady>max(abs(float(m.get("target",0)))*0.03,5):suggestions.append("稳态误差较大：在输出未饱和的前提下，小幅增加 Ki，并注意积分饱和。")
        if not suggestions:suggestions.append("当前响应没有明显单一问题。建议保存本次结果，再用小步长改一个参数后进行 A/B 对比。")
        lines=["阶跃分析完成",f"Score: {score:.2f}",f"Rise: {m.get('rise_time_s')} s",f"Overshoot: {ov:.2f}%",f"Settling: {m.get('settling_time_s')} s",f"Steady error: {steady:.2f}",f"RMSE: {float(m.get('rmse',0)):.2f}","","本地调参建议："]+["• "+x for x in suggestions]
        self.out.setPlainText("\n".join(lines))
        self.store.add("speed_step","速度环阶跃",self.config.get("vehicle",{}).get("display_name",""),parameters=p,metrics={**m,"score":score},samples=sm)
