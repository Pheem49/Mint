import React, { memo, useMemo } from 'react'
import type { AgentProgress } from '../../types'
import WeatherCard from './WeatherCard'
import StockCard from './StockCard'
import CalculationCard from './CalculationCard'
import ImageSearchCard from './ImageSearchCard'
import ImageGenCard from './ImageGenCard'

export interface ToolResultCardsProps {
  progress?: AgentProgress[]
  aiText?: string
}

function extractFallbackCardJson(
  progress: AgentProgress[] | undefined,
  aiText: string | undefined,
  fenceMarkers: string[],
  actions: Set<string>,
  regex: RegExp,
): any | null {
  if (aiText && fenceMarkers.some((marker) => aiText.includes(marker))) {
    return null
  }
  for (const event of progress || []) {
    if (event.type === 'ToolEnd') {
      const actionName = event.data?.action || (event.data as any)?.name || ''
      if (actions.has(actionName)) {
        const res = event.data.result || (event.data as any)?.output || ''
        const match = typeof res === 'string' && res.match(regex)
        if (match && match[1]) {
          try {
            return JSON.parse(match[1])
          } catch {}
        }
      }
    }
  }
  return null
}

const WEATHER_ACTIONS = new Set(['weather'])
const STOCK_ACTIONS = new Set(['stock'])
const CALC_ACTIONS = new Set(['calculation'])
const IMAGE_SEARCH_ACTIONS = new Set(['image_search'])
const IMAGE_GEN_ACTIONS = new Set(['generate_image', 'image_studio.generate', 'image_generate'])

export const ToolResultCards = memo(function ToolResultCards({ progress, aiText }: ToolResultCardsProps) {
  const weatherData = useMemo(
    () =>
      extractFallbackCardJson(
        progress,
        aiText,
        ['```weather_json', '```weather-json'],
        WEATHER_ACTIONS,
        /```weather_json\s*([\s\S]*?)\s*```/,
      ),
    [progress, aiText],
  )

  const stockData = useMemo(
    () =>
      extractFallbackCardJson(
        progress,
        aiText,
        ['```stock_json', '```stock-json'],
        STOCK_ACTIONS,
        /```stock_json\s*([\s\S]*?)\s*```/,
      ),
    [progress, aiText],
  )

  const calcData = useMemo(
    () =>
      extractFallbackCardJson(
        progress,
        aiText,
        ['```calculation_json', '```calculation-json'],
        CALC_ACTIONS,
        /```calculation_json\s*([\s\S]*?)\s*```/,
      ),
    [progress, aiText],
  )

  const imageSearchData = useMemo(
    () =>
      extractFallbackCardJson(
        progress,
        aiText,
        ['```image_search_json', '```image-search-json'],
        IMAGE_SEARCH_ACTIONS,
        /```image_search_json\s*([\s\S]*?)\s*```/,
      ),
    [progress, aiText],
  )

  const imageGenData = useMemo(
    () =>
      extractFallbackCardJson(
        progress,
        aiText,
        ['```image_gen_json', '```image-gen-json'],
        IMAGE_GEN_ACTIONS,
        /```image_gen_json\s*([\s\S]*?)\s*```/,
      ),
    [progress, aiText],
  )

  if (!weatherData && !stockData && !calcData && !imageSearchData && !imageGenData) {
    return null
  }

  return (
    <>
      {weatherData && <WeatherCard data={weatherData} />}
      {stockData && <StockCard data={stockData} />}
      {calcData && <CalculationCard data={calcData} />}
      {imageSearchData && <ImageSearchCard data={imageSearchData} />}
      {imageGenData && <ImageGenCard data={imageGenData} />}
    </>
  )
})

export default ToolResultCards
