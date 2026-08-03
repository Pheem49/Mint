import React, { useMemo } from 'react';
import {
  Sun,
  CloudSun,
  Cloud,
  Cloudy,
  CloudFog,
  CloudDrizzle,
  CloudRain,
  CloudSunRain,
  CloudRainWind,
  CloudSnow,
  Snowflake,
  CloudLightning,
  Droplet,
  type LucideIcon,
} from 'lucide-react';

export interface WeatherData {
  location: string;
  current: {
    time?: string;
    temperature_2m: number;
    relative_humidity_2m: number;
    apparent_temperature: number;
    is_day?: number;
    precipitation?: number;
    weather_code: number;
    wind_speed_10m: number;
  };
  daily?: {
    time: string[];
    weather_code: number[];
    temperature_2m_max: number[];
    temperature_2m_min: number[];
    precipitation_probability_max?: number[];
  };
  timezone?: string;
}

const getWeatherInfo = (code: number, isDay: boolean = true) => {
  const weatherMap: Record<number, { icon: LucideIcon; description: string; gradient: string }> = {
    0: {
      icon: Sun,
      description: 'Clear Sky',
      gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e3a8a, #1d4ed8 40%, #0284c7 80%, #0369a1)',
    },
    1: {
      icon: CloudSun,
      description: 'Mainly Clear',
      gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e3a8a, #1d4ed8 40%, #0284c7 80%, #0369a1)',
    },
    2: {
      icon: Cloud,
      description: 'Partly Cloudy',
      gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e293b, #334155 45%, #0284c7 85%)',
    },
    3: {
      icon: Cloudy,
      description: 'Overcast',
      gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #334155 50%, #475569)',
    },
    45: { icon: CloudFog, description: 'Foggy', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e293b, #475569)' },
    48: { icon: CloudFog, description: 'Rime Fog', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e293b, #475569)' },
    51: { icon: CloudDrizzle, description: 'Light Drizzle', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0c4a6e, #0284c7 50%, #0369a1)' },
    53: { icon: CloudDrizzle, description: 'Drizzle', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0c4a6e, #0284c7 50%, #0369a1)' },
    55: { icon: CloudRain, description: 'Heavy Drizzle', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #0369a1 60%)' },
    61: { icon: CloudRain, description: 'Light Rain', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0c4a6e, #0284c7 50%, #0369a1)' },
    63: { icon: CloudRain, description: 'Rain', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #0284c7 50%, #0369a1)' },
    65: { icon: CloudRain, description: 'Heavy Rain', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #1e3a8a 60%, #0284c7)' },
    71: { icon: CloudSnow, description: 'Light Snow', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e293b, #38bdf8 70%)' },
    73: { icon: CloudSnow, description: 'Snow', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e293b, #38bdf8 70%)' },
    75: { icon: Snowflake, description: 'Heavy Snow', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #0284c7 70%)' },
    80: { icon: CloudSunRain, description: 'Light Showers', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0c4a6e, #0284c7 50%, #0369a1)' },
    81: { icon: CloudRain, description: 'Showers', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #0284c7 50%, #0369a1)' },
    82: { icon: CloudRainWind, description: 'Heavy Showers', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #1e3a8a 50%, #312e81)' },
    95: { icon: CloudLightning, description: 'Thunderstorm', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #1e1b4b 50%, #312e81)' },
    96: { icon: CloudLightning, description: 'Thunderstorm & Hail', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #1e1b4b 60%)' },
    99: { icon: CloudLightning, description: 'Severe Thunderstorm', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #0f172a, #1e1b4b 60%)' },
  };

  return weatherMap[code] || { icon: CloudSun, description: 'Weather', gradient: 'radial-gradient(ellipse 150% 100% at 50% 100%, #1e3a8a, #0284c7)' };
};

export default function WeatherCard({ data }: { data: WeatherData }) {
  const current = data.current;
  const daily = data.daily;
  const isDay = current?.is_day !== 0;

  const weatherInfo = useMemo(() => {
    return getWeatherInfo(current?.weather_code || 0, isDay);
  }, [current?.weather_code, isDay]);

  const forecast = useMemo(() => {
    if (!daily?.time || daily.time.length === 0) return [];
    return daily.time.slice(1, 6).map((time, idx) => {
      const date = new Date(time);
      const dayName = date.toLocaleDateString('en-US', { weekday: 'short' });
      const code = daily.weather_code[idx + 1] || 0;
      const info = getWeatherInfo(code, true);
      const high = Math.round(daily.temperature_2m_max[idx + 1] || 0);
      const low = Math.round(daily.temperature_2m_min[idx + 1] || 0);
      const prob = daily.precipitation_probability_max ? daily.precipitation_probability_max[idx + 1] : 0;

      return {
        day: dayName,
        icon: info.icon,
        high,
        low,
        precipitation: prob,
      };
    });
  }, [daily]);

  if (!current) return null;

  const formattedDate = current.time
    ? new Date(current.time).toLocaleString('en-US', { weekday: 'short', hour: 'numeric', minute: '2-digit' })
    : new Date().toLocaleString('en-US', { weekday: 'short', hour: 'numeric', minute: '2-digit' });

  return (
    <div
      style={{
        background: weatherInfo.gradient,
        color: '#ffffff',
        borderRadius: '12px',
        padding: '16px 20px',
        margin: '12px 0',
        boxShadow: '0 10px 25px -5px rgba(0, 0, 0, 0.4)',
        position: 'relative',
        overflow: 'hidden',
        fontFamily: 'system-ui, -apple-system, sans-serif',
      }}
    >
      {/* Header Info */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
          <weatherInfo.icon size={42} strokeWidth={1.75} style={{ flexShrink: 0 }} />
          <div>
            <div style={{ display: 'flex', alignItems: 'baseline', gap: '4px' }}>
              <span style={{ fontSize: '36px', fontWeight: 'bold', letterSpacing: '-1px' }}>
                {Math.round(current.temperature_2m)}°
              </span>
              <span style={{ fontSize: '18px', opacity: 0.9 }}>C</span>
            </div>
            <p style={{ margin: '2px 0 0', fontSize: '14px', fontWeight: 500, opacity: 0.95 }}>
              {weatherInfo.description}
            </p>
          </div>
        </div>
        <div style={{ textAlign: 'right' }}>
          {daily?.temperature_2m_max?.[0] !== undefined && (
            <p style={{ margin: 0, fontSize: '13px', fontWeight: 600, opacity: 0.9 }}>
              {Math.round(daily.temperature_2m_max[0])}° {Math.round(daily.temperature_2m_min[0])}°
            </p>
          )}
        </div>
      </div>

      {/* Location & Time */}
      <div
        style={{
          borderBottom: '1px solid rgba(255, 255, 255, 0.2)',
          paddingBottom: '10px',
          marginBottom: '12px',
        }}
      >
        <h4 style={{ margin: 0, fontSize: '17px', fontWeight: 700, letterSpacing: '-0.3px' }}>{data.location}</h4>
        <p style={{ margin: '2px 0 0', fontSize: '12px', opacity: 0.8 }}>{formattedDate}</p>
      </div>

      {/* 5-Day Forecast Grid */}
      {forecast.length > 0 && (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: `repeat(${forecast.length}, 1fr)`,
            gap: '8px',
            marginBottom: '14px',
            paddingBottom: '12px',
            borderBottom: '1px solid rgba(255, 255, 255, 0.2)',
          }}
        >
          {forecast.map((day, idx) => (
            <div
              key={idx}
              style={{
                display: 'flex',
                flexDirection: 'column',
                alignItems: 'center',
                background: 'rgba(255, 255, 255, 0.12)',
                backdropFilter: 'blur(4px)',
                borderRadius: '8px',
                padding: '8px 4px',
              }}
            >
              <span style={{ fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>{day.day}</span>
              <day.icon size={20} strokeWidth={2} style={{ marginBottom: '4px' }} />
              <div style={{ fontSize: '11px', display: 'flex', gap: '3px' }}>
                <span style={{ fontWeight: 700 }}>{day.high}°</span>
                <span style={{ opacity: 0.65 }}>{day.low}°</span>
              </div>
              {day.precipitation !== undefined && day.precipitation > 0 && (
                <span style={{ fontSize: '10px', opacity: 0.8, marginTop: '2px', display: 'inline-flex', alignItems: 'center', gap: '2px' }}>
                  <Droplet size={10} strokeWidth={2.5} /> {day.precipitation}%
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Metrics Row */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '8px' }}>
        <div
          style={{
            background: 'rgba(255, 255, 255, 0.12)',
            backdropFilter: 'blur(4px)',
            borderRadius: '8px',
            padding: '8px 10px',
          }}
        >
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.75, textTransform: 'uppercase', letterSpacing: '0.5px' }}>Wind</p>
          <p style={{ margin: '2px 0 0', fontSize: '13px', fontWeight: 700 }}>{Math.round(current.wind_speed_10m)} km/h</p>
        </div>

        <div
          style={{
            background: 'rgba(255, 255, 255, 0.12)',
            backdropFilter: 'blur(4px)',
            borderRadius: '8px',
            padding: '8px 10px',
          }}
        >
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.75, textTransform: 'uppercase', letterSpacing: '0.5px' }}>Humidity</p>
          <p style={{ margin: '2px 0 0', fontSize: '13px', fontWeight: 700 }}>{Math.round(current.relative_humidity_2m)}%</p>
        </div>

        <div
          style={{
            background: 'rgba(255, 255, 255, 0.12)',
            backdropFilter: 'blur(4px)',
            borderRadius: '8px',
            padding: '8px 10px',
          }}
        >
          <p style={{ margin: 0, fontSize: '10px', opacity: 0.75, textTransform: 'uppercase', letterSpacing: '0.5px' }}>Feels Like</p>
          <p style={{ margin: '2px 0 0', fontSize: '13px', fontWeight: 700 }}>{Math.round(current.apparent_temperature)}°C</p>
        </div>
      </div>
    </div>
  );
}
