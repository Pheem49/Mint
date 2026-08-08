import React, { useRef, useState } from 'react'
import { useAuthUser } from '../../../shared/components/AuthGate'
import { authUploadAvatar } from '../../tauri'

interface ProfileTabProps {
  name: string
  setName: (val: string) => void
  imageUrl: string
  setImageUrl: (val: string) => void
}

export default function ProfileTab({ name, setName, imageUrl, setImageUrl }: ProfileTabProps) {
  const { user, avatarUrl, refreshUser } = useAuthUser()
  const [isUploading, setIsUploading] = useState(false)
  const [uploadError, setUploadError] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleFileChange = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ''
    if (!file) return
    if (!file.type.startsWith('image/')) {
      setUploadError('Please select a valid image file.')
      return
    }
    setIsUploading(true)
    setUploadError(null)
    try {
      const dataUri = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(reader.result as string)
        reader.onerror = reject
        reader.readAsDataURL(file)
      })
      const updated = await authUploadAvatar(dataUri, file.name)
      setImageUrl(updated.image || '')
      refreshUser(updated)
    } catch (error) {
      console.error('Avatar upload error:', error)
      setUploadError('Failed to upload image.')
    } finally {
      setIsUploading(false)
    }
  }

  if (!user) {
    return (
      <div className="tab-pane active animate-fade-in">
        <section className="setting-section">
          <p className="section-description">Sign in to manage your profile.</p>
        </section>
      </div>
    )
  }

  return (
    <div className="tab-pane active animate-fade-in" style={{ display: 'flex', flexDirection: 'column', gap: '24px' }}>
      <section className="setting-section">
        <div className="section-heading">
          <div>
            <p className="section-kicker">Account</p>
            <h2 className="section-title">Profile</h2>
          </div>
        </div>

        <div className="form-grid single">
          <div className="setting-row wide">
            <label>Profile Logo / Avatar</label>
            <div className="memory-field-container">
              <p className="hint">Click the picture or use the upload button to select an image from your device. Uploads are saved immediately.</p>
              <div className="profile-avatar-row">
                <input
                  type="file"
                  ref={fileInputRef}
                  accept="image/*"
                  style={{ display: 'none' }}
                  onChange={handleFileChange}
                />
                <div className="profile-avatar" onClick={() => fileInputRef.current?.click()}>
                  {avatarUrl ? (
                    <img src={avatarUrl} alt={name || 'User'} />
                  ) : (
                    (name?.[0] || user.email?.[0] || 'U').toUpperCase()
                  )}
                </div>
                <div className="profile-avatar-meta">
                  <button
                    type="button"
                    className="btn-secondary btn-small"
                    onClick={() => fileInputRef.current?.click()}
                    disabled={isUploading}
                  >
                    {isUploading ? 'Uploading…' : 'Upload Photo'}
                  </button>
                  {user.email && <span className="hint">{user.email}</span>}
                </div>
              </div>
              {uploadError && <p className="profile-message-error">{uploadError}</p>}
            </div>
          </div>

          <div className="setting-row">
            <label>Display Name</label>
            <div className="memory-field-container">
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Enter your name"
              />
            </div>
          </div>

          <div className="setting-row">
            <label>Profile Image URL</label>
            <div className="memory-field-container">
              <input
                type="text"
                value={imageUrl}
                onChange={(e) => setImageUrl(e.target.value)}
                placeholder="https://example.com/avatar.png or /api/avatar?key=..."
              />
            </div>
          </div>
        </div>
      </section>
    </div>
  )
}
