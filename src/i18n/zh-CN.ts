export const zhCN = {
  appName: "Codex 用量监控器",
  nav: {
    overview: "总览",
    projects: "项目",
    models: "模型",
    history: "历史记录",
    settings: "设置",
    usage: "用量",
    limits: "额度",
  },
  status: {
    ready: "就绪",
    available: "可用",
    unavailable: "不可用",
    connected: "已连接",
    running: "运行中",
    loading: "正在加载",
    checking: "正在检查",
    refreshing: "正在刷新",
    saving: "正在保存",
    error: "错误",
    unknown: "未知",
    noData: "暂无数据",
  },
  range: {
    today: "今天",
    sevenDays: "7 天",
    thirtyDays: "30 天",
    all: "全部",
  },
} as const;

export function formatNumber(value: number): string {
  return value.toLocaleString("zh-CN");
}

export function formatDateTime(value: number | null | undefined): string {
  return value ? new Date(value * 1000).toLocaleString("zh-CN") : "--";
}
