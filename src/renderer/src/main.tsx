import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import '@shared/fonts'
import './index.css'
import { installTauriAdapters } from './tauri'
import * as tauriPlatform from './tauri'
import {
  checkoutRemoteGitBranch, createGitBranch, createWorkspaceFile, createWorkspaceFolder,
  deleteWorkspaceItem, getGitBranchInfo, getGitGraph, getWorkspaceSnapshot, switchGitBranch,
} from './tauri'
import { installRendererPlatform, installWorkspacePlatform } from '@shared/platform'

installTauriAdapters()
installRendererPlatform(tauriPlatform)
installWorkspacePlatform({
  getWorkspaceSnapshot, getGitBranchInfo, switchGitBranch, createGitBranch,
  checkoutRemoteGitBranch, getGitGraph, createWorkspaceFile, createWorkspaceFolder, deleteWorkspaceItem,
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
