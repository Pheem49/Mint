import clearDay from './clear-day.svg?url';
import clearNight from './clear-night.svg?url';
import cloudyDay from './cloudy-1-day.svg?url';
import cloudyNight from './cloudy-1-night.svg?url';
import fogDay from './fog-day.svg?url';
import fogNight from './fog-night.svg?url';
import frostDay from './frost-day.svg?url';
import frostNight from './frost-night.svg?url';
import rainy1Day from './rainy-1-day.svg?url';
import rainy1Night from './rainy-1-night.svg?url';
import rainy2Day from './rainy-2-day.svg?url';
import rainy2Night from './rainy-2-night.svg?url';
import rainy3Day from './rainy-3-day.svg?url';
import rainy3Night from './rainy-3-night.svg?url';
import snowy1Day from './snowy-1-day.svg?url';
import snowy1Night from './snowy-1-night.svg?url';
import snowy2Day from './snowy-2-day.svg?url';
import snowy2Night from './snowy-2-night.svg?url';
import snowy3Day from './snowy-3-day.svg?url';
import snowy3Night from './snowy-3-night.svg?url';
import scatteredThunderstormsDay from './scattered-thunderstorms-day.svg?url';
import scatteredThunderstormsNight from './scattered-thunderstorms-night.svg?url';
import severeThunderstorm from './severe-thunderstorm.svg?url';

// WMO weather codes (as used by Open-Meteo) mapped to a day/night icon pair.
const WEATHER_ICON_MAP: Record<number, { day: string; night: string }> = {
  0: { day: clearDay, night: clearNight }, // Clear sky
  1: { day: clearDay, night: clearNight }, // Mainly clear
  2: { day: cloudyDay, night: cloudyNight }, // Partly cloudy
  3: { day: cloudyDay, night: cloudyNight }, // Overcast
  45: { day: fogDay, night: fogNight }, // Fog
  48: { day: frostDay, night: frostNight }, // Rime fog
  51: { day: rainy1Day, night: rainy1Night }, // Light drizzle
  53: { day: rainy1Day, night: rainy1Night }, // Drizzle
  55: { day: rainy2Day, night: rainy2Night }, // Heavy drizzle
  61: { day: rainy1Day, night: rainy1Night }, // Light rain
  63: { day: rainy2Day, night: rainy2Night }, // Rain
  65: { day: rainy3Day, night: rainy3Night }, // Heavy rain
  71: { day: snowy1Day, night: snowy1Night }, // Light snow
  73: { day: snowy2Day, night: snowy2Night }, // Snow
  75: { day: snowy3Day, night: snowy3Night }, // Heavy snow
  80: { day: rainy1Day, night: rainy1Night }, // Light showers
  81: { day: rainy2Day, night: rainy2Night }, // Showers
  82: { day: rainy3Day, night: rainy3Night }, // Heavy showers
  95: { day: scatteredThunderstormsDay, night: scatteredThunderstormsNight }, // Thunderstorm
  96: { day: severeThunderstorm, night: severeThunderstorm }, // Thunderstorm & hail
  99: { day: severeThunderstorm, night: severeThunderstorm }, // Severe thunderstorm
};

export function getWeatherIconUrl(code: number, isDay: boolean): string {
  const entry = WEATHER_ICON_MAP[code] ?? WEATHER_ICON_MAP[2];
  return isDay ? entry.day : entry.night;
}
