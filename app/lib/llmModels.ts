import catalog from '../../src-tauri/resources/llm-models.json'
export const providers = catalog.providers
export const catalogUpdatedAt = catalog.updated_at
export const defaultModel = (provider: string) => providers.find(row => row.id === provider)?.models[0] ?? ''
export function modelChoices(suggestions: string[], selected: string) {
  return [...new Set([...suggestions, ...(selected ? [selected] : [])])]
}
export function geminiTextModels(rawModels: unknown): string[] {
  if (!Array.isArray(rawModels)) return []
  return [...new Set(rawModels.filter(row => row && typeof row.name === 'string' && Array.isArray(row.supportedGenerationMethods) && row.supportedGenerationMethods.includes('generateContent'))
    .map(row => row.name.replace(/^models\//, '') as string)
    .filter(name => name.startsWith('gemini-') && !/(?:image|tts|audio|live|transcribe|omni|robotics)/i.test(name)))]
}
