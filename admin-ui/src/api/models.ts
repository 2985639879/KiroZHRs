import axios from 'axios'
import { storage } from '@/lib/storage'
import { md5 } from '@/lib/crypto'

// 创建 axios 实例
const api = axios.create({
  baseURL: '/api/admin',
  headers: {
    'Content-Type': 'application/json',
  },
})

// 请求拦截器添加 API Key（MD5 哈希）
api.interceptors.request.use(async (config) => {
  const apiKey = storage.getApiKey()
  if (apiKey) {
    // 发送 API Key 的 MD5 哈希而不是明文
    const hashedKey = md5(apiKey)
    config.headers['x-api-key'] = hashedKey
  }
  return config
})

export interface ModelInfo {
  id: string
  object: string
  created: number
  owned_by: string
  display_name: string
  model_type: string
  max_tokens: number
  context_window: number
  supports_image: boolean
  input_modalities: string[]
  pricing?: {
    input: number
    output: number
  }
  available_accounts: string[]
}

export interface ModelsResponse {
  models: ModelInfo[]
  total: number
}

export interface RefreshResponse {
  success: boolean
  message: string
  total_accounts?: number
  failed_accounts?: number
  account_id?: number
  model_count?: number
}

// 获取所有模型列表
export async function getModels(): Promise<ModelsResponse> {
  const { data } = await api.get<ModelsResponse>('/models')
  return data
}

// 刷新所有账号的模型列表
export async function refreshAllModels(): Promise<RefreshResponse> {
  const { data } = await api.post<RefreshResponse>('/models/refresh')
  return data
}

// 刷新指定账号的模型列表
export async function refreshAccountModels(accountId: number): Promise<RefreshResponse> {
  const { data } = await api.post<RefreshResponse>(`/models/refresh/${accountId}`)
  return data
}
