import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app';
import { restoreCampaignManifest, startCampaignManifestPersistence } from './features/bench/campaign-manifest-store';
import {
  restoreSingleRunManifest,
  startSingleRunManifestPersistence,
} from './features/bench/single-run-manifest-store';
import { restoreHangarState, startHangarPersistence } from './features/hangar/hangar-persistence';
import './styles.css';

const root = document.getElementById('root');
if (!root) throw new Error('A3S agent evaluation application root was not found');

restoreHangarState();
restoreCampaignManifest();
restoreSingleRunManifest();
startHangarPersistence();
startCampaignManifestPersistence();
startSingleRunManifestPersistence();

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>
);
