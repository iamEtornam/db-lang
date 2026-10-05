import { describe, it, expect } from 'vitest'
import { providers, defaultModel, modelChoices, geminiTextModels } from './llmModels'
describe('model catalog', () => {
  it('offers current defaults and preserves saved/custom IDs without duplicates', () => {
    expect(providers.map(row => row.id)).toEqual(['gemini','openai','anthropic','ollama','deepseek','groq','custom'])
    expect(defaultModel('gemini')).toBe('gemini-3.8-flash')
    expect(defaultModel('openai')).toBe('gpt-6.1-sol')
    expect(defaultModel('custom')).toBe('')
    expect(modelChoices(['current','current'], 'saved-snapshot')).toEqual(['current','saved-snapshot'])
    expect(modelChoices(['current'], 'current')).toEqual(['current'])
    expect(modelChoices([], 'deployment-name')).toEqual(['deployment-name'])
    expect(providers.find(row => row.id === 'deepseek')?.models).not.toContain('deepseek-chat')
  })
  it('filters malformed Gemini catalogs and non-text models', () => {
    expect(geminiTextModels(null)).toEqual([])
    const row = (name: string) => ({name,supportedGenerationMethods:['generateContent']})
    expect(geminiTextModels([row('models/gemini-3.8-flash'),row('models/gemini-3.8-flash'),row('models/gemini-3.8-flash-tts'),row('models/gemini-3-pro-image'),row('models/gemini-3.8-live'),row('models/gemini-omni-1.1-flash'),{name:'embedding',supportedGenerationMethods:['embedContent']},{name:7},null])).toEqual(['gemini-3.8-flash'])
  })
})
