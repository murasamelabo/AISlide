import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './studio.css'
import Studio from './Studio.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Studio />
  </StrictMode>,
)
