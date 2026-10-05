import catalog from '../../src-tauri/resources/llm-models.json'
export const providers = catalog.providers
export const catalogUpdatedAt = catalog.updated_at
export const defaultModel = (provider: string) => providers.find(row => row.id === provider)?.models[0] ?? ''
export function modelChoices(suggestions: string[], selected: string) {
  return [...new Set([...suggestions, ...(selected ? [selected] : [])])]
}
export function geminiTextModels(rawModels: unknown): string[] {
  if (!Array.isArray(rawModels)) return []
  return [...new Set(rawModels.flatMap(row => typeof row === 'string' ? [row] : row && typeof row.name === 'string' && Array.isArray(row.supportedGenerationMethods) && row.supportedGenerationMethods.includes('generateContent') ? [row.name as string] : [])
    .map(name => name.replace(/^models\//, ''))
    .filter(name => name.startsWith('gemini-') && !/(?:image|tts|audio|live|transcribe|omni|robotics)/i.test(name)))]
}
