import { useState, useEffect } from 'react'
import { toast } from 'sonner'
import { Pencil } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { CredentialStatusItem } from '@/types/api'
import { useUpdateCredential } from '@/hooks/use-credentials'

interface EditCredentialDialogProps {
  credential: CredentialStatusItem
}

export function EditCredentialDialog({ credential }: EditCredentialDialogProps) {
  const [open, setOpen] = useState(false)
  const [email, setEmail] = useState('')
  const [kiroApiKey, setKiroApiKey] = useState('')
  const [authMethod, setAuthMethod] = useState<string>('')
  const [region, setRegion] = useState('')
  const [authRegion, setAuthRegion] = useState('')
  const [apiRegion, setApiRegion] = useState('')
  const [proxyUrl, setProxyUrl] = useState('')
  const [proxyUsername, setProxyUsername] = useState('')
  const [proxyPassword, setProxyPassword] = useState('')
  const [endpoint, setEndpoint] = useState('')
  const [subscriptionTitle, setSubscriptionTitle] = useState('')

  const updateCredential = useUpdateCredential()

  // 初始化表单值
  useEffect(() => {
    if (open) {
      setEmail(credential.email || '')
      setKiroApiKey('') // 出于安全考虑，不显示现有的 API Key
      setAuthMethod(credential.authMethod || 'social')
      setRegion('')
      setAuthRegion('')
      setApiRegion('')
      setProxyUrl(credential.proxyUrl || '')
      setProxyUsername('')
      setProxyPassword('')
      setEndpoint(credential.endpoint || '')
      setSubscriptionTitle(credential.subscriptionTitle || '')
    }
  }, [open, credential])

  const handleSubmit = () => {
    // 构建更新数据（只包含非空字段）
    const updateData: Record<string, unknown> = {
      id: credential.id,
    }

    if (email) updateData.email = email
    if (kiroApiKey) updateData.kiroApiKey = kiroApiKey
    if (authMethod) updateData.authMethod = authMethod
    if (region) updateData.region = region
    if (authRegion) updateData.authRegion = authRegion
    if (apiRegion) updateData.apiRegion = apiRegion
    if (proxyUrl) updateData.proxyUrl = proxyUrl
    if (proxyUsername) updateData.proxyUsername = proxyUsername
    if (proxyPassword) updateData.proxyPassword = proxyPassword
    if (endpoint) updateData.endpoint = endpoint
    if (subscriptionTitle) updateData.subscriptionTitle = subscriptionTitle

    updateCredential.mutate(updateData, {
      onSuccess: (res) => {
        toast.success(res.message || '更新成功')
        setOpen(false)
      },
      onError: (err) => {
        toast.error('更新失败: ' + (err as Error).message)
      },
    })
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm" variant="outline">
          <Pencil className="h-4 w-4 mr-1" />
          编辑
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>编辑账号信息</DialogTitle>
          <DialogDescription>
            修改账号 #{credential.id} 的配置信息
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {/* 基本信息 */}
          <div className="space-y-2">
            <Label htmlFor="email">邮箱</Label>
            <Input
              id="email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="user@example.com"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="subscriptionTitle">订阅等级</Label>
            <Input
              id="subscriptionTitle"
              value={subscriptionTitle}
              onChange={(e) => setSubscriptionTitle(e.target.value)}
              placeholder="KIRO POWER"
            />
          </div>

          {/* 认证信息 */}
          <div className="space-y-2">
            <Label htmlFor="authMethod">认证方式</Label>
            <Select value={authMethod} onValueChange={setAuthMethod}>
              <SelectTrigger id="authMethod">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="social">Social</SelectItem>
                <SelectItem value="idc">IdC</SelectItem>
                <SelectItem value="api_key">API Key</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {authMethod === 'api_key' && (
            <div className="space-y-2">
              <Label htmlFor="kiroApiKey">Kiro API Key</Label>
              <Input
                id="kiroApiKey"
                type="password"
                value={kiroApiKey}
                onChange={(e) => setKiroApiKey(e.target.value)}
                placeholder="ksk_xxxxxxxx（留空则不修改）"
              />
              <p className="text-xs text-muted-foreground">
                出于安全考虑，现有的 API Key 不会显示。输入新值以更新，留空则保持不变。
              </p>
            </div>
          )}

          {/* 区域配置 */}
          <div className="grid grid-cols-3 gap-4">
            <div className="space-y-2">
              <Label htmlFor="region">Region</Label>
              <Input
                id="region"
                value={region}
                onChange={(e) => setRegion(e.target.value)}
                placeholder="us-east-1"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="authRegion">Auth Region</Label>
              <Input
                id="authRegion"
                value={authRegion}
                onChange={(e) => setAuthRegion(e.target.value)}
                placeholder="us-east-1"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="apiRegion">API Region</Label>
              <Input
                id="apiRegion"
                value={apiRegion}
                onChange={(e) => setApiRegion(e.target.value)}
                placeholder="us-east-1"
              />
            </div>
          </div>

          {/* 端点配置 */}
          <div className="space-y-2">
            <Label htmlFor="endpoint">端点</Label>
            <Input
              id="endpoint"
              value={endpoint}
              onChange={(e) => setEndpoint(e.target.value)}
              placeholder="ide"
            />
          </div>

          {/* 代理配置 */}
          <div className="space-y-2">
            <Label htmlFor="proxyUrl">代理 URL</Label>
            <Input
              id="proxyUrl"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
              placeholder="http://proxy.example.com:8080"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="proxyUsername">代理用户名</Label>
              <Input
                id="proxyUsername"
                value={proxyUsername}
                onChange={(e) => setProxyUsername(e.target.value)}
                placeholder="username"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="proxyPassword">代理密码</Label>
              <Input
                id="proxyPassword"
                type="password"
                value={proxyPassword}
                onChange={(e) => setProxyPassword(e.target.value)}
                placeholder="password"
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            取消
          </Button>
          <Button
            onClick={handleSubmit}
            disabled={updateCredential.isPending}
          >
            {updateCredential.isPending ? '更新中...' : '保存'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
