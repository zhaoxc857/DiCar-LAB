import logging
import time
import yaml
from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QWidget,QVBoxLayout,QHBoxLayout,QLineEdit,QPushButton,QComboBox,QPlainTextEdit,QMessageBox,QFileDialog
from core.paths import data_root
ROOT=data_root()
log=logging.getLogger(__name__)

class ProfileManager(QWidget):
    def __init__(self,bus,transport,config):
        super().__init__();self.transport=transport;self.config=config;self.params=config.get("parameters",[]);self.values={};vid=config.get("vehicle",{}).get("id","default");self.dir=ROOT/"profiles"/str(vid);self.dir.mkdir(parents=True,exist_ok=True)
        root=QVBoxLayout(self);row=QHBoxLayout();self.name=QLineEdit("stable");self.combo=QComboBox();read=QPushButton("读取全部参数");save=QPushButton("保存当前方案");load=QPushButton("加载并下发");card=QPushButton("导出调参卡片");read.clicked.connect(self._read_all);row.addWidget(self.name);row.addWidget(read);row.addWidget(save);row.addWidget(self.combo);row.addWidget(load);row.addWidget(card);root.addLayout(row)
        self.preview=QPlainTextEdit();self.preview.setReadOnly(True);root.addWidget(self.preview);save.clicked.connect(self._save);load.clicked.connect(self._load);card.clicked.connect(self._export_card);self.combo.currentTextChanged.connect(self._preview);bus.ack.connect(self._ack);self._refresh()
    def _ack(self,k,v):self.values[str(k)]=v
    def _read_all(self):
        for p in self.params:self.transport.get_param(p.get("key",""))
    def _refresh(self):self.combo.clear();self.combo.addItems([p.stem for p in sorted(self.dir.glob("*.yaml"))])
    def _save(self):
        missing=[str(p.get("key","")) for p in self.params if str(p.get("key","")) and str(p.get("key","")) not in self.values]
        if missing:
            answer=QMessageBox.question(self,"参数方案",
                f"有 {len(missing)}/{len(self.params)} 个参数本次会话尚未回读（未点过「读取全部」或 MCU 未确认）。\n"
                f"直接保存会得到不完整的方案。仍要保存吗？")
            if answer!=QMessageBox.StandardButton.Yes:return
        name=self.name.text().strip() or "profile";path=self.dir/(name+".yaml");obj={"vehicle":self.config.get("vehicle",{}),"parameters":self.values};path.write_text(yaml.safe_dump(obj,allow_unicode=True,sort_keys=False),encoding="utf-8");self._refresh();self.combo.setCurrentText(name)
    def _preview(self,name):
        p=self.dir/(name+".yaml");self.preview.setPlainText(p.read_text(encoding="utf-8") if p.exists() else "")
    def _load(self):
        p=self.dir/(self.combo.currentText()+".yaml")
        if not p.exists():return
        obj=yaml.safe_load(p.read_text(encoding="utf-8")) or {}
        failed=[]
        for k,v in (obj.get("parameters") or {}).items():
            try:self.transport.set_param(k,float(v))
            except Exception as e:failed.append(f"{k}={v}（{e}）")
        if failed:
            log.warning("参数方案下发失败: "+"; ".join(failed))
            QMessageBox.warning(self,"参数方案","以下参数未能下发（已跳过，其余已开始下发）：\n"+"\n".join(failed))
        else:
            QMessageBox.information(self,"参数方案","已开始下发；请在 ACK/协议监视器确认全部参数生效。")

    def _export_card(self):
        """调参卡片 = 参数方案 + 固件版本信息，可直接被「加载并下发」消费。"""
        try:
            from core.firmware_store import FirmwareStore
            latest=[]
            store=FirmwareStore()
            vid=str(self.config.get("vehicle",{}).get("id",""))
            rows=store.list(vehicle=vid,limit=5)
            latest=[{k:row[k] for k in ("note","family","sha256","result")} for row in rows[:1]]
        except Exception:
            latest=[]
        obj={
            "vehicle":self.config.get("vehicle",{}),
            "parameters":self.values,
            "tuning_card":{
                "exported_at":time.strftime("%Y-%m-%d %H:%M:%S"),
                "note":self.name.text().strip(),
                "firmware_history":latest,
            },
        }
        default=str(ROOT/"reports"/f"tuning_card_{self.name.text().strip() or 'profile'}.yaml")
        path,_=QFileDialog.getSaveFileName(self,"导出调参卡片",default,"YAML (*.yaml)")
        if not path:return
        try:
            with open(path,"w",encoding="utf-8") as handle:
                yaml.safe_dump(obj,handle,allow_unicode=True,sort_keys=False)
        except OSError as exc:
            QMessageBox.warning(self,"调参卡片",f"导出失败：{exc}");return
        QMessageBox.information(self,"调参卡片",
            f"已导出：{path}\n卡片兼容「加载并下发」（对方把文件放进参数方案目录或直接分享均可）。")
