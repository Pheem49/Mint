import React, { lazy, memo, Suspense, useMemo } from 'react'
import type { AgentProgress } from '../../types'

const WeatherCard = lazy(() => import('./WeatherCard'))
const StockCard = lazy(() => import('./StockCard'))
const CalculationCard = lazy(() => import('./CalculationCard'))
const ImageSearchCard = lazy(() => import('./ImageSearchCard'))
const ImageGenCard = lazy(() => import('./ImageGenCard'))

export interface ToolResultCardsProps {
  progress?: AgentProgress[]
  aiText?: string
}

const WEATHER_RE = /```weather_json\s*([\s\S]*?)\s*```/
const STOCK_RE = /```stock_json\s*([\s\S]*?)\s*```/
const CALC_RE = /```calculation_json\s*([\s\S]*?)\s*```/
const IMAGE_SEARCH_RE = /```image_search_json\s*([\s\S]*?)\s*```/
const IMAGE_GEN_RE = /```image_gen_json\s*([\s\S]*?)\s*```/
const IMAGE_GEN_ACTIONS = new Set(['generate_image', 'image_studio.generate', 'image_generate'])

function parseMatchedJson(result: unknown, regex: RegExp): any | null {
  const match = typeof result === 'string' && result.match(regex)
  if (match && match[1]) {
    try {
      return JSON.parse(match[1])
    } catch {}
  }
  return null
}

export function extractToolCardPayloads(progress?: AgentProgress[], aiText?: string) {
  const skipWeather = Boolean(aiText && (aiText.includes('```weather_json') || aiText.includes('```weather-json')))
  const skipStock = Boolean(aiText && (aiText.includes('```stock_json') || aiText.includes('```stock-json')))
  const skipCalc = Boolean(aiText && (aiText.includes('```calculation_json') || aiText.includes('```calculation-json')))
  const skipImageSearch = Boolean(aiText && (aiText.includes('```image_search_json') || aiText.includes('```image-search-json')))
  const skipImageGen = Boolean(aiText && (aiText.includes('```image_gen_json') || aiText.includes('```image-gen-json')))

  let weatherData: any = null
  let stockData: any = null
  let calcData: any = null
  let imageSearchData: any = null
  let imageGenData: any = null

  for (const event of progress || []) {
    if (event.type !== 'ToolEnd') continue
    const actionName = event.data?.action || (event.data as any)?.name || ''
    const res = event.data?.result || (event.data as any)?.output || ''
    if (!skipWeather && !weatherData && actionName === 'weather') {
      weatherData = parseMatchedJson(res, WEATHER_RE)
    } else if (!skipStock && !stockData && actionName === 'stock') {
      stockData = parseMatchedJson(res, STOCK_RE)
    } else if (!skipCalc && !calcData && actionName === 'calculation') {
      calcData = parseMatchedJson(res, CALC_RE)
    } else if (!skipImageSearch && !imageSearchData && actionName === 'image_search') {
      imageSearchData = parseMatchedJson(res, IMAGE_SEARCH_RE)
    } else if (!skipImageGen && !imageGenData && IMAGE_GEN_ACTIONS.has(actionName)) {
      imageGenData = parseMatchedJson(res, IMAGE_GEN_RE)
    }
  }

  return { weatherData, stockData, calcData, imageSearchData, imageGenData }
}

export const ToolResultCards = memo(function ToolResultCards({ progress, aiText }: ToolResultCardsProps) {
  const { weatherData, stockData, calcData, imageSearchData, imageGenData } = useMemo(
    () => extractToolCardPayloads(progress, aiText),
    [progress, aiText],
  )

  if (!weatherData && !stockData && !calcData && !imageSearchData && !imageGenData) {
    return null
  }

  return (
    <Suspense fallback={null}>
      {weatherData && <WeatherCard data={weatherData} />}
      {stockData && <StockCard data={stockData} />}
      {calcData && <CalculationCard data={calcData} />}
      {imageSearchData && <ImageSearchCard data={imageSearchData} />}
      {imageGenData && <ImageGenCard data={imageGenData} />}
    </Suspense>
  )
})

export default ToolResultCards
