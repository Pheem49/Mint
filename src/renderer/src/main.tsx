import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import '@shared/fonts'
import './index.css'
import { installTauriAdapters } from './tauri'

installTauriAdapters()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
