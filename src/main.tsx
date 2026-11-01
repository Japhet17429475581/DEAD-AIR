import React from 'react';
import { createRoot } from 'react-dom/client';
import './styles.css';
import { App } from './App';
import { SkullPreview } from './components/skull/SkullPreview';

const developmentPreview = import.meta.env.DEV && new URLSearchParams(location.search).has('skull-preview');
createRoot(document.getElementById('root')!).render(<React.StrictMode>{developmentPreview ? <SkullPreview /> : <App />}</React.StrictMode>);
