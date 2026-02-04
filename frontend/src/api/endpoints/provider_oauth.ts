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
