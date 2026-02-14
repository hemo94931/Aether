// API 格式常量
export const API_FORMATS = {
  // 新模式：endpoint signature key（family:kind，全小写）
  CLAUDE: 'claude:chat',
  CLAUDE_CLI: 'claude:cli',
  OPENAI: 'openai:chat',
  OPENAI_CLI: 'openai:cli',
  OPENAI_VIDEO: 'openai:video',
  GEMINI: 'gemini:chat',
  GEMINI_CLI: 'gemini:cli',
  GEMINI_VIDEO: 'gemini:video',
} as const

export type APIFormat = typeof API_FORMATS[keyof typeof API_FORMATS]

// API 格式显示名称映射（按品牌分组：Chat 在前，CLI/Video 在后）
export const API_FORMAT_LABELS: Record<string, string> = {
  [API_FORMATS.CLAUDE]: 'Claude Chat',
  [API_FORMATS.CLAUDE_CLI]: 'Claude CLI',
  [API_FORMATS.OPENAI]: 'OpenAI Chat',
  [API_FORMATS.OPENAI_CLI]: 'OpenAI CLI',
  [API_FORMATS.OPENAI_VIDEO]: 'OpenAI Video',
  [API_FORMATS.GEMINI]: 'Gemini Chat',
  [API_FORMATS.GEMINI_CLI]: 'Gemini CLI',
  [API_FORMATS.GEMINI_VIDEO]: 'Gemini Video',
  // legacy 兼容（仅用于展示历史数据）
  CLAUDE: 'Claude Chat',
  CLAUDE_CLI: 'Claude CLI',
  OPENAI: 'OpenAI Chat',
  OPENAI_CLI: 'OpenAI CLI',
  OPENAI_VIDEO: 'OpenAI Video',
  GEMINI: 'Gemini Chat',
  GEMINI_CLI: 'Gemini CLI',
  GEMINI_VIDEO: 'Gemini Video',
}

// API 格式缩写映射（用于空间紧凑的显示场景）
export const API_FORMAT_SHORT: Record<string, string> = {
  [API_FORMATS.OPENAI]: 'O',
  [API_FORMATS.OPENAI_CLI]: 'OC',
  [API_FORMATS.OPENAI_VIDEO]: 'OV',
  [API_FORMATS.CLAUDE]: 'C',
  [API_FORMATS.CLAUDE_CLI]: 'CC',
  [API_FORMATS.GEMINI]: 'G',
  [API_FORMATS.GEMINI_CLI]: 'GC',
  [API_FORMATS.GEMINI_VIDEO]: 'GV',
  // legacy 兼容（仅用于展示历史数据）
  OPENAI: 'O',
  OPENAI_CLI: 'OC',
  OPENAI_VIDEO: 'OV',
  CLAUDE: 'C',
  CLAUDE_CLI: 'CC',
  GEMINI: 'G',
  GEMINI_CLI: 'GC',
  GEMINI_VIDEO: 'GV',
}

// API 格式排序顺序（统一的显示顺序）
export const API_FORMAT_ORDER: string[] = [
  API_FORMATS.OPENAI,
  API_FORMATS.OPENAI_CLI,
  API_FORMATS.OPENAI_VIDEO,
  API_FORMATS.CLAUDE,
  API_FORMATS.CLAUDE_CLI,
  API_FORMATS.GEMINI,
  API_FORMATS.GEMINI_CLI,
  API_FORMATS.GEMINI_VIDEO,
]

// 工具函数：按标准顺序排序 API 格式数组
export function sortApiFormats(formats: string[]): string[] {
  return [...formats].sort((a, b) => {
    const aIdx = API_FORMAT_ORDER.indexOf(a)
    const bIdx = API_FORMAT_ORDER.indexOf(b)
    if (aIdx === -1 && bIdx === -1) return 0
    if (aIdx === -1) return 1
    if (bIdx === -1) return -1
    return aIdx - bIdx
  })
}
