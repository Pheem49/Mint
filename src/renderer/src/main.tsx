import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import '@shared/fonts'
import './index.css'
import { installTauriAdapters } from './tauri'
import * as tauriPlatform from './tauri'
import {
  checkoutRemoteGitBranch, createGitBranch, createWorkspaceFile, createWorkspaceFolder,
  deleteWorkspaceItem, moveWorkspaceItem, startHtmlPreview, listWorkspaceHistory, undoWorkspaceAction, getGitBranchInfo, getGitGraph, getWorkspaceGitDiff, getWorkspaceSnapshot, switchGitBranch,
} from './tauri'
import { installRendererPlatform, installWorkspacePlatform } from '@shared/platform'

installTauriAdapters()
installRendererPlatform(tauriPlatform)
installWorkspacePlatform({
  getWorkspaceSnapshot, getWorkspaceGitDiff, getGitBranchInfo, switchGitBranch, createGitBranch,
  checkoutRemoteGitBranch, getGitGraph, createWorkspaceFile, createWorkspaceFolder, deleteWorkspaceItem,
  moveWorkspaceItem, startHtmlPreview, listWorkspaceHistory, undoWorkspaceAction,
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
