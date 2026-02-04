import client from '../client'

export interface ProviderOAuthSupportedType {
  provider_type: string
  display_name: string
  scopes: string[]
  redirect_uri: string
  authorize_url: string
  token_url: string
  use_pkce: boolean
}

export interface ProviderOAuthStartResponse {
  authorization_url: string
  redirect_uri: string
  provider_type: string
  instructions: string
}

export interface ProviderOAuthCompleteRequest {
  callback_url: string
}

export interface ProviderOAuthCompleteResponse {
  provider_type: string
  expires_at?: number | null
  has_refresh_token: boolean
  email?: string | null
}

export async function getProviderOAuthSupportedTypes(): Promise<ProviderOAuthSupportedType[]> {
  const resp = await client.get('/api/admin/provider-oauth/supported-types')
  return resp.data
}

export async function startProviderOAuth(keyId: string): Promise<ProviderOAuthStartResponse> {
  const resp = await client.post(`/api/admin/provider-oauth/keys/${keyId}/start`)
  return resp.data
}

export async function completeProviderOAuth(
  keyId: string,
  data: ProviderOAuthCompleteRequest
): Promise<ProviderOAuthCompleteResponse> {
  const resp = await client.post(`/api/admin/provider-oauth/keys/${keyId}/complete`, data)
  return resp.data
}

export async function refreshProviderOAuth(keyId: string): Promise<ProviderOAuthCompleteResponse> {
  const resp = await client.post(`/api/admin/provider-oauth/keys/${keyId}/refresh`)
  return resp.data
}

// Provider-level OAuth (不需要预先创建 key)

export interface ProviderOAuthCompleteRequest {
  callback_url: string
  name?: string
}

export interface ProviderOAuthCompleteResponseWithKey {
  key_id: string
  provider_type: string
  expires_at?: number | null
  has_refresh_token: boolean
  email?: string | null
}

export async function startProviderLevelOAuth(providerId: string): Promise<ProviderOAuthStartResponse> {
  const resp = await client.post(`/api/admin/provider-oauth/providers/${providerId}/start`)
  return resp.data
}

export async function completeProviderLevelOAuth(
  providerId: string,
  data: ProviderOAuthCompleteRequest
): Promise<ProviderOAuthCompleteResponseWithKey> {
  const resp = await client.post(`/api/admin/provider-oauth/providers/${providerId}/complete`, data)
  return resp.data
}
