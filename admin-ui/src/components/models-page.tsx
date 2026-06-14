import { useState, useEffect, useMemo } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { RefreshCw, Server, Clock, Image as ImageIcon, DollarSign, Search, Filter } from 'lucide-react'
import { toast } from 'sonner'
import { getModels, refreshAllModels, type ModelInfo } from '@/api/models'

export function ModelsPage() {
  const [models, setModels] = useState<ModelInfo[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [isRefreshing, setIsRefreshing] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [filterType, setFilterType] = useState<string>('all')

  const loadModels = async () => {
    try {
      setIsLoading(true)
      const data = await getModels()
      setModels(data.models)
    } catch (error) {
      toast.error('Failed to load models')
      console.error('Failed to load models:', error)
    } finally {
      setIsLoading(false)
    }
  }

  const handleRefreshAll = async () => {
    try {
      setIsRefreshing(true)
      const result = await refreshAllModels()
      toast.success(result.message)
      // 刷新完成后重新加载模型列表
      await loadModels()
    } catch (error) {
      toast.error('Failed to refresh models')
      console.error('Failed to refresh models:', error)
    } finally {
      setIsRefreshing(false)
    }
  }

  useEffect(() => {
    loadModels()
  }, [])

  // 过滤和搜索
  const filteredModels = useMemo(() => {
    return models.filter((model) => {
      // 搜索过滤
      const matchesSearch = searchQuery === '' ||
        model.display_name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        model.id.toLowerCase().includes(searchQuery.toLowerCase())

      // 类型过滤
      const matchesType = filterType === 'all' || model.model_type === filterType

      return matchesSearch && matchesType
    })
  }, [models, searchQuery, filterType])

  // 获取所有模型类型
  const modelTypes = useMemo(() => {
    const types = new Set(models.map(m => m.model_type))
    return Array.from(types)
  }, [models])

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <RefreshCw className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h2 className="text-3xl font-bold tracking-tight">模型列表</h2>
          <p className="text-muted-foreground">
            管理和查看所有账号的可用AI模型
          </p>
        </div>
        <Button
          onClick={handleRefreshAll}
          disabled={isRefreshing}
          className="gap-2"
        >
          <RefreshCw className={`h-4 w-4 ${isRefreshing ? 'animate-spin' : ''}`} />
          刷新全部
        </Button>
      </div>

      {/* 搜索和过滤 */}
      {models.length > 0 && (
        <div className="flex gap-4 flex-col sm:flex-row">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="搜索模型名称或 ID..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10"
            />
          </div>
          <div className="flex gap-2">
            <Button
              variant={filterType === 'all' ? 'default' : 'outline'}
              size="sm"
              onClick={() => setFilterType('all')}
            >
              全部 ({models.length})
            </Button>
            {modelTypes.map((type) => (
              <Button
                key={type}
                variant={filterType === type ? 'default' : 'outline'}
                size="sm"
                onClick={() => setFilterType(type)}
              >
                {type.charAt(0).toUpperCase() + type.slice(1)} ({models.filter(m => m.model_type === type).length})
              </Button>
            ))}
          </div>
        </div>
      )}

      {models.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Server className="h-12 w-12 text-muted-foreground mb-4" />
            <p className="text-lg font-medium">暂无可用模型</p>
            <p className="text-sm text-muted-foreground">
              点击"刷新全部"从你的账号加载模型
            </p>
          </CardContent>
        </Card>
      ) : filteredModels.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Filter className="h-12 w-12 text-muted-foreground mb-4" />
            <p className="text-lg font-medium">没有匹配的模型</p>
            <p className="text-sm text-muted-foreground">
              尝试调整搜索或筛选条件
            </p>
            <Button
              variant="outline"
              className="mt-4"
              onClick={() => {
                setSearchQuery('')
                setFilterType('all')
              }}
            >
              清除筛选
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-4">
          <div className="text-sm text-muted-foreground">
            显示 {filteredModels.length} / {models.length} 个模型
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {filteredModels.map((model) => (
            <Card key={model.id} className="hover:shadow-lg transition-shadow">
              <CardHeader>
                <div className="flex justify-between items-start">
                  <div className="flex-1">
                    <CardTitle className="text-lg mb-1">
                      {model.display_name}
                    </CardTitle>
                    <CardDescription className="text-xs font-mono">
                      {model.id}
                    </CardDescription>
                  </div>
                  <Badge variant="secondary">{model.model_type}</Badge>
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Clock className="h-4 w-4" />
                  <span>
                    创建时间: {new Date(model.created * 1000).toLocaleDateString('zh-CN')}
                  </span>
                </div>

                <div className="space-y-1 text-sm">
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">最大输出:</span>
                    <span className="font-medium">{model.max_tokens.toLocaleString()} tokens</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">上下文窗口:</span>
                    <span className="font-medium">{model.context_window.toLocaleString()} tokens</span>
                  </div>
                </div>

                <div className="flex gap-2 flex-wrap">
                  {model.supports_image && (
                    <Badge variant="outline" className="gap-1">
                      <ImageIcon className="h-3 w-3" />
                      视觉
                    </Badge>
                  )}
                  {model.input_modalities?.map((modality) => (
                    <Badge key={modality} variant="outline">
                      {modality === 'text' ? '文本' : modality === 'image' ? '图像' : modality}
                    </Badge>
                  ))}
                </div>

                {model.pricing && model.pricing.input !== undefined && model.pricing.output !== undefined && (
                  <div className="pt-2 border-t space-y-1 text-xs">
                    <div className="flex items-center gap-1 text-muted-foreground">
                      <DollarSign className="h-3 w-3" />
                      <span>定价 (每百万tokens)</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">输入:</span>
                      <span className="font-medium">${model.pricing.input.toFixed(2)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">输出:</span>
                      <span className="font-medium">${model.pricing.output.toFixed(2)}</span>
                    </div>
                  </div>
                )}

                <div className="pt-2 border-t">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">可用账号:</span>
                    <Badge variant="default">{model.available_accounts.length}</Badge>
                  </div>
                  {model.available_accounts.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-1">
                      {model.available_accounts.slice(0, 5).map((accountId) => (
                        <Badge key={accountId} variant="secondary" className="text-xs">
                          #{accountId}
                        </Badge>
                      ))}
                      {model.available_accounts.length > 5 && (
                        <Badge variant="secondary" className="text-xs">
                          +{model.available_accounts.length - 5} 更多
                        </Badge>
                      )}
                    </div>
                  )}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
        </div>
      )}
    </div>
  )
}
