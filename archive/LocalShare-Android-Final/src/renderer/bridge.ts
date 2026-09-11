import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { LocalShareAPI, LocalShareState } from '../shared/types';

export async function createApi(): Promise<LocalShareAPI> {
  const unavailableState = (message: string): LocalShareState => ({
    self: { id: '', name: '', fingerprint: '', platform: '' },
    peers: [],
    transfers: [],
    settings: { deviceName: '', discoverable: true, downloadFolder: '', askBeforeReceiving: true, networkInterface: 'Automatic', transferConcurrency: 2 },
    diagnostics: { interfaces: [], listenerPort: 0, mdns: 'error', peerCount: 0, protocolVersion: 1, internet: 'unknown', lastError: message },
  });
  const getState = async (): Promise<LocalShareState> => {
    for (;;) {
      try {
        return await invoke<LocalShareState>('get_state');
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        document.body.dataset.localshareError = message;
        if (message.includes('startup failed') || message.includes('startup timed out')) return unavailableState(message);
        await new Promise((resolve) => window.setTimeout(resolve, 250));
      }
    }
  };
  const api: LocalShareAPI = {
    getState,
    onState: (listener) => { let unlisten: (() => void) | undefined; void listen('core-state', (event) => listener(event.payload as Awaited<ReturnType<LocalShareAPI['getState']>>)).then((fn) => { unlisten = fn; }); return () => { unlisten?.(); }; },
    pair: (id) => invoke('pair', { peerId: id }),
    forgetPeer: (id) => invoke('forget_peer', { peerId: id }),
    blockPeer: (id, blocked) => invoke('block_peer', { peerId: id, blocked }),
    sendMessage: (id, body) => invoke('send_message', { peerId: id, body }),
    getMessages: (id, before) => invoke('get_messages', { peerId: id, before }),
    chooseFiles: () => invoke('choose_files'),
    sendFiles: (id, paths) => invoke('send_files', { peerId: id, paths }),
    acceptTransfer: (id, directory) => invoke('accept_transfer', { transferId: id, directory }),
    declineTransfer: (id) => invoke('decline_transfer', { transferId: id }),
    pauseTransfer: (id) => invoke('pause_transfer', { transferId: id }),
    resumeTransfer: (id) => invoke('resume_transfer', { transferId: id }),
    cancelTransfer: (id) => invoke('cancel_transfer', { transferId: id }),
    updateSettings: (settings) => invoke('update_settings', { settings }),
    connectManual: (host, port, fingerprint) => invoke('connect_manual', { host, port, fingerprint }),
  };
  return api;
}
