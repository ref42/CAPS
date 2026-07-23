<template>
    <transition @enter="onEnter" @leave="onLeave" :css="false">
        <div v-show="isIslandVisible" :class="['island-container', { 'has-music-border': isGlowBorderEnabled }]"
            @mousedown="handleMouseDown" @mousemove="handleMouseMove" @mouseup="handleMouseUp"
            @mouseleave="handleMouseLeave" @mouseenter="handleMouseEnter" :style="islandStyle"
            @contextmenu="handleRightClick">

            <div class="rainbow-border-glow" v-if="isGlowBorderEnabled" :style="{ opacity: glowOpacity }"></div>

            <div class="island-core-content" :class="{ 'panel-open': isIslandPanelOpen }" :style="coreContentStyle">
                <div class="inner-wrapper">
                    <transition mode="out-in" @enter="onInnerEnter" @leave="onInnerLeave" :css="false">
                        <div v-if="isMsgActive" class="msg-box" key="msg">
                            <div class="msg-avatar">
                                <img :src="currentMsgIcon" alt="消息图标" class="msg-avatar-img">
                            </div>
                            <div class="msg-text-wrapper">
                                <div class="msg-title">
                                    <span class="sender-name">{{ msgTitle }}</span>
                                    <span class="app-name">{{ msgAppName }}</span>
                                </div>
                                <div class="msg-body">{{ msgBody }}</div>
                            </div>
                        </div>

                        <div v-else-if="displaySysToast" class="system-toast-box" key="systoast">
                            <div v-if="sysToastType === 'app'" class="toast-icon app-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <circle cx="12" cy="12" r="10" stroke-width="2" stroke-linecap="round"
                                        stroke-linejoin="round" opacity="0.3" />
                                    <path d="M8 12.5l3 3 5-6" stroke-width="2.5" stroke-linecap="round"
                                        stroke-linejoin="round" />
                                </svg>
                            </div>

                            <div v-else-if="sysToastType === 'lock'" class="toast-icon sys-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <rect x="4" y="12" width="16" height="8" rx="2" ry="2" stroke-width="2"
                                        stroke-linecap="round" stroke-linejoin="round" />
                                    <path d="M8 12V9a4 4 0 0 1 8 0v3" stroke-width="2" stroke-linecap="round"
                                        stroke-linejoin="round" />
                                </svg>
                            </div>

                            <div v-else-if="sysToastType === 'unlock'" class="toast-icon sys-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <rect x="4" y="12" width="16" height="8" rx="2" ry="2" stroke-width="2"
                                        stroke-linecap="round" stroke-linejoin="round" />
                                    <path d="M8 12V9a4 4 0 0 1 8 0" stroke-width="2" stroke-linecap="round"
                                        stroke-linejoin="round" />
                                </svg>
                            </div>

                            <div v-else-if="sysToastType === 'battery-charge'" class="toast-icon battery-charge-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <rect x="2" y="7" width="16" height="10" rx="2" ry="2" stroke-width="2"
                                        stroke-linecap="round" stroke-linejoin="round" />
                                    <line x1="22" y1="11" x2="22" y2="13" stroke-width="2" stroke-linecap="round"
                                        stroke-linejoin="round" />
                                    <polygon points="11 7 8 12 12 12 11 17 14 12 10 12 11 7" stroke-width="1.5"
                                        stroke-linejoin="round" />
                                </svg>
                            </div>

                            <div v-else-if="sysToastType === 'battery-low'" class="toast-icon battery-low-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <rect x="2" y="7" width="16" height="10" rx="2" ry="2" stroke-width="2"
                                        stroke-linecap="round" stroke-linejoin="round" />
                                    <line x1="22" y1="11" x2="22" y2="13" stroke-width="2" stroke-linecap="round"
                                        stroke-linejoin="round" />
                                    <line x1="6" y1="12" x2="9" y2="12" stroke-width="4" stroke-linecap="round"
                                        stroke-linejoin="round" />
                                </svg>
                            </div>

                            <div v-else class="toast-icon sys-icon">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <circle cx="12" cy="12" r="10" stroke-width="2" stroke-linecap="round"
                                        stroke-linejoin="round" opacity="0.3" />
                                    <g transform="translate(6, 5.5) scale(0.5)">
                                        <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" stroke-width="4"
                                            stroke-linecap="round" stroke-linejoin="round" />
                                        <path d="M13.73 21a2 2 0 0 1-3.46 0" stroke-width="4" stroke-linecap="round"
                                            stroke-linejoin="round" />
                                    </g>
                                </svg>
                            </div>
                            <div class="toast-text">{{ sysToastText }}</div>
                        </div>

                        <div v-else-if="displayMusic" class="music-ctl-box" :class="{ 'expanded': isMusicExpanded }"
                            :key="'music_' + musicBoxKey" @click="expandMusic" style="cursor: pointer;">
                            <div class="music-top-row">
                                <div class="album-cover" :class="{ 'is-playing': isPlaying }">
                                    <div class="cover-inner"
                                        :style="coverUrl ? { backgroundImage: `url(${coverUrl})`, backgroundSize: 'cover' } : {}">
                                    </div>
                                </div>
                                <div class="music-info-mask-box" ref="maskBoxRef">
                                    <div class="music-info-text single-line" :class="{ 'fade-out': isMusicExpanded }"
                                        style="position: relative; width: 100%; height: 100%;">
                                        <transition name="lyric-fade">
                                            <span class="lyric-render-text" :key="currentTrackInfo">
                                                {{ currentTrackInfo }}
                                            </span>
                                        </transition>
                                    </div>
                                    <div class="music-info-text double-line" :class="{ 'fade-in': isMusicExpanded }">
                                        <div class="song-title">{{ currentSongName }}</div>
                                        <div class="song-artist">{{ currentArtistName }}</div>
                                    </div>
                                </div>
                            </div>
                            <transition name="fade">
                                <div class="music-controls" v-show="isMusicExpanded">
                                    <button class="ctl-btn" @click.stop="prevTrack">
                                        <svg viewBox="0 0 24 24" fill="currentColor">
                                            <path d="M6 6h2v12H6zm3.5 6l8.5 6V6z" />
                                        </svg>
                                    </button>
                                    <button class="ctl-btn play-btn" @click.stop="togglePlay">
                                        <svg v-if="isPlaying" viewBox="0 0 24 24" fill="currentColor">
                                            <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
                                        </svg>
                                        <svg v-else viewBox="0 0 24 24" fill="currentColor"
                                            style="transform: translateX(1px);">
                                            <path d="M8 5v14l11-7z" />
                                        </svg>
                                    </button>
                                    <button class="ctl-btn" @click.stop="nextTrack">
                                        <svg viewBox="0 0 24 24" fill="currentColor">
                                            <path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z" />
                                        </svg>
                                    </button>
                                </div>
                            </transition>
                        </div>

                        <div v-else-if="displaySpeed" class="speed-box" key="speed">
                            <transition name="speed-fade" mode="out-in">
                                <div v-if="isShowingUpload" class="speed-item" key="upload">
                                    <span :class="['label', { 'high-traffic': isHighUpload }]">⬆</span>
                                    <span class="value">{{ uploadSpeed }}</span>
                                </div>
                                <div v-else class="speed-item" key="download">
                                    <span :class="['label', { 'high-traffic': isHighDownload }]">⬇</span>
                                    <span class="value">{{ downloadSpeed }}</span>
                                </div>
                            </transition>
                        </div>
                    </transition>
                </div>

                <transition mode="out-in" @enter="onInnerEnter" @leave="onInnerLeave" :css="false">
                    <div v-if="showSpectrumIndicator" class="audio-spectrum"
                        :class="{ 'is-playing': isPlaying, 'expanded': isMusicExpanded }" key="spectrum">
                        <span class="bar" v-for="(val, index) in spectrumData" :key="index"
                            :style="{ transform: `scaleY(${val})` }"></span>
                    </div>

                    <div v-else :class="['status-dot', networkStatus]" key="dot"></div>
                </transition>

                <transition name="panel-material">
                    <section v-show="isIslandPanelOpen" class="island-hover-panel" @click.stop @mousedown.stop>
                        <div class="panel-tabs">
                            <button class="panel-tab" :class="{ active: islandPanelActiveTab === 'search' }"
                                @click="islandPanelActiveTab = 'search'" title="Search">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <circle cx="11" cy="11" r="7" stroke-width="2" />
                                    <path d="m20 20-3.5-3.5" stroke-width="2" stroke-linecap="round" />
                                </svg>
                            </button>
                            <button class="panel-tab" :class="{ active: islandPanelActiveTab === 'queue' }"
                                @click="islandPanelActiveTab = 'queue'" title="Queue">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <path d="M4 7h13M4 12h10M4 17h7" stroke-width="2" stroke-linecap="round" />
                                    <path d="M18 14v6l4-3-4-3Z" fill="currentColor" stroke="none" />
                                </svg>
                            </button>
                            <button class="panel-tab" :class="{ active: islandPanelActiveTab === 'stats' }"
                                @click="islandPanelActiveTab = 'stats'" title="Stats">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <path d="M5 19V9M12 19V5M19 19v-7" stroke-width="2" stroke-linecap="round" />
                                </svg>
                            </button>
                            <button class="panel-tab" :class="{ active: islandPanelActiveTab === 'settings' }"
                                @click="islandPanelActiveTab = 'settings'" title="Settings">
                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                    <path d="M12 8a4 4 0 1 1 0 8 4 4 0 0 1 0-8Z" stroke-width="2" />
                                    <path
                                        d="M4 12h2M18 12h2M12 4v2M12 18v2M6.3 6.3l1.4 1.4M16.3 16.3l1.4 1.4M17.7 6.3l-1.4 1.4M7.7 16.3l-1.4 1.4"
                                        stroke-width="2" stroke-linecap="round" />
                                </svg>
                            </button>
                        </div>

                        <div v-if="islandPanelActiveTab === 'search'" class="panel-section search-section">
                            <div class="search-row">
                                <input v-model="neteaseSearchQuery" class="panel-search-input" placeholder="Search NetEase"
                                    @focus="handlePanelInputFocus" @blur="handlePanelInputBlur"
                                    @keydown.enter="searchNetease" />
                                <button class="panel-icon-btn" @click="searchNetease" title="Search">
                                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor">
                                        <circle cx="11" cy="11" r="7" stroke-width="2" />
                                        <path d="m20 20-3.5-3.5" stroke-width="2" stroke-linecap="round" />
                                    </svg>
                                </button>
                            </div>
                            <div class="random-row">
                                <button class="panel-pill-btn" @click="loadRandomQueue(50)">Random 50</button>
                                <button class="panel-pill-btn" @click="loadRandomQueue(100)">Random 100</button>
                            </div>
                            <div class="panel-list">
                                <button v-for="song in searchResults" :key="songId(song)" class="song-row"
                                    @click="addSongToQueue(song)">
                                    <img v-if="song.cover" :src="song.cover" class="song-thumb" />
                                    <span v-else class="song-thumb song-thumb-empty"></span>
                                    <span class="song-meta">
                                        <span class="song-name">{{ song.name }}</span>
                                        <span class="song-sub">{{ song.artist || 'Unknown artist' }}</span>
                                    </span>
                                    <span class="song-action">+</span>
                                </button>
                                <div v-if="!searchResults.length" class="panel-empty">{{ neteaseStatus || 'Search songs, then add them to the queue.' }}</div>
                            </div>
                        </div>

                        <div v-else-if="islandPanelActiveTab === 'queue'" class="panel-section queue-section">
                            <div class="queue-toolbar">
                                <span>{{ cloudQueue.length }} tracks</span>
                                <button class="panel-pill-btn" @click="clearCloudQueue">Clear</button>
                            </div>
                            <div class="panel-list queue-list">
                                <button v-for="(song, index) in cloudQueue" :key="`${songId(song)}-${index}`"
                                    class="song-row" :class="{ active: index === cloudQueueIndex }"
                                    @click="playCloudQueueIndex(index)">
                                    <img v-if="song.cover" :src="song.cover" class="song-thumb" />
                                    <span v-else class="song-thumb song-thumb-empty"></span>
                                    <span class="song-meta">
                                        <span class="song-name">{{ song.name }}</span>
                                        <span class="song-sub">{{ song.artist || 'Unknown artist' }}</span>
                                    </span>
                                </button>
                                <div v-if="!cloudQueue.length" class="panel-empty">Queue is empty.</div>
                            </div>
                        </div>

                        <div v-else-if="islandPanelActiveTab === 'stats'" class="panel-section stats-section">
                            <div class="live-speed-grid">
                                <div class="live-speed-card">
                                    <span>Upload</span>
                                    <strong>{{ uploadSpeed }}</strong>
                                </div>
                                <div class="live-speed-card">
                                    <span>Download</span>
                                    <strong>{{ downloadSpeed }}</strong>
                                </div>
                            </div>
                            <div class="stat-mini-grid">
                                <div>
                                    <span>Total up</span>
                                    <strong>{{ formatBytesValue(totalUpload) }} {{ formatBytesUnit(totalUpload) }}</strong>
                                </div>
                                <div>
                                    <span>Total down</span>
                                    <strong>{{ formatBytesValue(totalDownload) }} {{ formatBytesUnit(totalDownload) }}</strong>
                                </div>
                                <div>
                                    <span>This month</span>
                                    <strong>{{ formatBytesValue(monthTraffic) }} {{ formatBytesUnit(monthTraffic) }}</strong>
                                </div>
                            </div>
                            <div class="network-row">
                                <span :class="['network-dot-large', networkStatus]"></span>
                                <span>{{ networkStatus === 'good' ? 'Network stable' : networkStatus === 'warning' ? 'Network busy' : 'Network offline' }}</span>
                            </div>
                        </div>

                        <div v-else class="panel-section settings-section">
                            <div class="setting-line">
                                <span>Opacity</span>
                                <input class="mini-range" type="range" min="20" max="100" v-model.number="islandOpacity"
                                    @input="syncIslandOpacityQuick" />
                            </div>
                            <div class="setting-line">
                                <span>Media island</span>
                                <label class="panel-switch">
                                    <input type="checkbox" v-model="isMusicCtlEnabled" @change="syncQuickSettings">
                                    <span></span>
                                </label>
                            </div>
                            <div class="setting-line">
                                <span>Glow border</span>
                                <label class="panel-switch">
                                    <input type="checkbox" v-model="isGlowBorderEnabled" @change="syncQuickSettings">
                                    <span></span>
                                </label>
                            </div>
                            <div class="setting-line">
                                <span>Messages</span>
                                <label class="panel-switch">
                                    <input type="checkbox" v-model="isMsgNotifyEnabled" @change="syncQuickSettings">
                                    <span></span>
                                </label>
                            </div>
                            <div class="setting-line">
                                <span>Quiet</span>
                                <label class="panel-switch">
                                    <input type="checkbox" v-model="isMsgModeEnabled" @change="syncQuickSettings">
                                    <span></span>
                                </label>
                            </div>
                            <div class="setting-line">
                                <span>Auto hide</span>
                                <label class="panel-switch">
                                    <input type="checkbox" v-model="isAutoHideEnabled" @change="syncQuickSettings">
                                    <span></span>
                                </label>
                            </div>
                            <div class="setting-line">
                                <span>Taskbar</span>
                                <label class="panel-switch">
                                    <input type="checkbox" v-model="isPinnedToTaskbar" @change="syncQuickSettings">
                                    <span></span>
                                </label>
                            </div>
                            <div class="setting-line">
                                <span>Theme</span>
                                <div class="segmented">
                                    <button :class="{ active: islandTheme === 'black' }"
                                        @click="setIslandThemeQuick('black')">Dark</button>
                                    <button :class="{ active: islandTheme === 'white' }"
                                        @click="setIslandThemeQuick('white')">Light</button>
                                </div>
                            </div>
                            <div class="setting-line">
                                <span>Motion</span>
                                <div class="segmented">
                                    <button :class="{ active: nsdSpringStyle === 'stiff' }"
                                        @click="setSpringStyleQuick('stiff')">Fast</button>
                                    <button :class="{ active: nsdSpringStyle === 'bouncy' }"
                                        @click="setSpringStyleQuick('bouncy')">Soft</button>
                                </div>
                            </div>
                            <div class="panel-empty status-line">{{ neteaseStatus }}</div>
                        </div>
                    </section>
                </transition>
            </div>
        </div>
    </transition>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch, nextTick, type CSSProperties } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow, currentMonitor, PhysicalPosition, LogicalPosition, PhysicalSize } from '@tauri-apps/api/window'; import { Menu, MenuItem } from '@tauri-apps/api/menu';
import { listen, emit } from '@tauri-apps/api/event';
import { t, currentLanguage, type AppLanguage } from '../i18n';

const isIslandVisible = ref(false);
const isMenuOpen = ref(false);


watch(isIslandVisible, (newVal) => {
    emit('island-status-sync', { visible: newVal });
});


const isAutoHideEnabled = ref(localStorage.getItem('nsd_autohide_fs') === 'true');

let wasVisibleBeforeFullscreen = false;


const isShowingUpload = ref(false);
let speedCycleTimer: number | null = null;


const currentWidth = ref(150);
const currentHeight = ref(34);
const isMsgActive = ref(false);
const msgTitle = ref('');
const msgAppName = ref('');
const msgBody = ref('');
const msgAumid = ref('');


const isMediaActive = ref(true); 
let isFirstMediaCheck = true;    
let isNewlyEnabled = false;


const displaySysToast = ref(false);
const sysToastText = ref('');
const sysToastType = ref<'app' | 'sys' | 'battery-charge' | 'battery-low' | 'lock' | 'unlock'>('app');
const toastQueue = ref<{ text: string, type: 'app' | 'sys' | 'battery-charge' | 'battery-low' | 'lock' | 'unlock' }[]>([]);
let isProcessingToast = false;


const processToastQueue = async () => {
    
    if (isProcessingToast || toastQueue.value.length === 0) return;

    
    if (isMsgActive.value) return;

    isProcessingToast = true;
    const nextToast = toastQueue.value.shift();

    if (nextToast) {
        sysToastText.value = nextToast.text;
        sysToastType.value = nextToast.type;
        displaySysToast.value = true;

        
        await new Promise(resolve => setTimeout(resolve, 2000));

        displaySysToast.value = false;
        
        await new Promise(resolve => setTimeout(resolve, 200));
    }

    isProcessingToast = false;
    processToastQueue(); 
};


watch(displaySysToast, (newVal) => {
    if (newVal) {
        
        animateIslandSize(260, 42);
    } else {
        
        
        if (!isMsgActive.value && !isMusicExpanded.value && !isMusicExpanding.value) {
            const { w, h } = getBaseSize();
            animateIslandSize(w, h);
        }
    }
});


const showToast = (text: string, type: 'app' | 'sys' | 'battery-charge' | 'battery-low' | 'lock' | 'unlock' = 'app') => {
    toastQueue.value.push({ text, type });
    processToastQueue();
};


watch(isMsgActive, (newVal) => {
    if (!newVal) {
        processToastQueue();
    }
});


const isMusicExpanded = ref(false);
const isMusicExpanding = ref(false); 
const isIslandPanelOpen = ref(false);
const islandPanelActiveTab = ref<'search' | 'queue' | 'stats' | 'settings'>('search');
const isPanelInputFocused = ref(false);
const isPointerInsideIsland = ref(false);
let musicExpandAnimTimer: number | null = null; 
let hoverOpenTimer: number | null = null;
let hoverCloseTimer: number | null = null;
let hoverWatchdogTimer: number | null = null;


const islandOpacity = ref(Number(localStorage.getItem('nsd_island_opacity') || '100'));


const islandTheme = ref(localStorage.getItem('nsd_island_theme') || 'black');


const nsdBaseWidth = ref(Number(localStorage.getItem('nsd_base_width')) || 150);
const nsdBaseHeight = ref(Number(localStorage.getItem('nsd_base_height')) || 34);
const nsdMusicBaseWidth = ref(Number(localStorage.getItem('nsd_music_base_width')) || 260);
const nsdMusicExpandedWidth = ref(Number(localStorage.getItem('nsd_music_expanded_width')) || 320);
const nsdMsgExpandedWidth = ref(Number(localStorage.getItem('nsd_msg_expanded_width')) || 360);
const nsdBorderRadius = ref(Number(localStorage.getItem('nsd_border_radius')) || 100);
const nsdSpringStyle = ref(localStorage.getItem('nsd_spring_style') || 'stiff');
const isAlwaysOnTop = ref(localStorage.getItem('nsd_always_on_top') !== 'false');


const isExpandedSize = computed(() => isMusicExpanded.value || isIslandPanelOpen.value || isMsgActive.value);


const islandStyle = computed<CSSProperties>(() => {
    const linear = islandOpacity.value / 100;
    const alpha = Math.pow(linear, 1 / 2.2);
    const baseStyle = islandTheme.value === 'white' ? {
        backgroundColor: `rgba(255, 255, 255, ${alpha})`,
        color: '#000000'
    } : {
        backgroundColor: `rgba(0, 0, 0, ${alpha})`,
        color: '#ffffff'
    };

    return {
        ...baseStyle,
        width: '100%',   
        height: '100%',  
        
        borderRadius: isExpandedSize.value ? '24px' : `${nsdBorderRadius.value}px`,
        position: 'relative',
    };
});


const coreContentStyle = computed(() => {
    const linear = islandOpacity.value / 100;
    const alpha = Math.pow(linear, 1 / 2.2);

    const innerRadiusValue = Math.max(nsdBorderRadius.value - 2, 8);
    const innerRadius = isExpandedSize.value ? '22px' : `${innerRadiusValue}px`;

    if (islandTheme.value === 'white') {
        return {
            backgroundColor: `rgba(255, 255, 255, ${alpha})`,
            borderRadius: innerRadius
        };
    }
    return {
        backgroundColor: `rgba(0, 0, 0, ${alpha})`,
        borderRadius: innerRadius
    };
});

const glowOpacity = computed(() => {
    const linear = islandOpacity.value / 100;
    return Math.pow(linear, 1 / 2.2);
});

const uploadSpeed = ref('0 KB/s');
const downloadSpeed = ref('0 KB/s');
const trafficData = ref<Record<string, { up: number; down: number }>>({});


const isHighDownload = ref(false);
const isHighUpload = ref(false);


const networkStatus = ref<'good' | 'warning' | 'error'>('good');


const isMusicCtlEnabled = ref(localStorage.getItem('nsd_music_ctrl') === 'true');
const isPlaying = ref(false);

const parsedLyrics = ref<{ time: number; text: string }[]>([]);
const currentBaseInfo = ref(''); 

const localPositionMs = ref(0);
let lastTickTime = performance.now();

const lyricQueue = ref<string[]>([]);
let lastLyricChangeTime = 0;
let currentMatchedIndex = -1;


const parseLrc = (lrcStr: string) => {
    const lines = lrcStr.split('\n');
    const result: { time: number; text: string }[] = [];
    const timeReg = /\[(\d{2}):(\d{2})\.(\d{2,3})\]/;

    for (const line of lines) {
        const match = timeReg.exec(line);
        if (match) {
            const min = parseInt(match[1]);
            const sec = parseInt(match[2]);
            const msStr = match[3].length === 2 ? match[3] + '0' : match[3];
            const ms = parseInt(msStr);
            const time = min * 60000 + sec * 1000 + ms;
            const text = line.replace(timeReg, '').trim();

            
            const realText = text.replace(/[\s\u200B-\u200D\uFEFF\u3000]/g, '');

            if (realText.length > 0 && !text.includes('纯音乐') && text !== 'lrc' && text !== '//') {
                result.push({ time, text });
            }
        }
    }
    return result.sort((a, b) => a.time - b.time);
};


const isGlowBorderEnabled = ref(localStorage.getItem('nsd_glow_border') === 'true');


const spectrumData = ref([0.35, 0.35, 0.35, 0.35, 0.35]);
let spectrumTimer: number;


const coverUrl = ref('');
const coverCache = new Map<string, string>();


const isPinnedToTaskbar = ref(localStorage.getItem('nsd_pin_taskbar') === 'true');

const isPositionLocked = ref(localStorage.getItem('nsd_position_locked') === 'true');

const isMsgModeEnabled = ref(localStorage.getItem('nsd_msg_mode') === 'true');
const isMsgNotifyEnabled = ref(localStorage.getItem('nsd_msg_notify') === 'true');


const displaySpeed = computed(() => !isMsgActive.value && !displaySysToast.value && (!isMusicCtlEnabled.value || !isMediaActive.value));
const displayMusic = computed(() => !isMsgActive.value && !displaySysToast.value && isMusicCtlEnabled.value && isMediaActive.value);


const getBaseSize = () => {
    if (displaySpeed.value) return { w: nsdBaseWidth.value, h: nsdBaseHeight.value };
    return { w: nsdMusicBaseWidth.value, h: Math.max(nsdBaseHeight.value + 8, 42) };
};


watch([displaySpeed, displayMusic], () => {
    
    if (!isMsgActive.value && !displaySysToast.value && !isMusicExpanded.value && !isMusicExpanding.value) {
        const { w, h } = getBaseSize();
        animateIslandSize(w, h);
    }
});


const showSpectrumIndicator = computed(() => {
    return isMusicCtlEnabled.value && isMediaActive.value;
});

type CloudSong = {
    id: string | number;
    name: string;
    artist?: string;
    album?: string;
    cover?: string;
    duration?: number;
    fee?: number;
};

type CloudSongUrl = {
    url?: string | null;
    playable?: boolean;
    trial?: boolean;
    quality?: string;
    reason?: string;
    message?: string;
    loggedIn?: boolean;
    vipLabel?: string;
};

const neteaseSearchQuery = ref('');
const searchResults = ref<CloudSong[]>([]);
const neteaseStatus = ref('');
const cloudQueue = ref<CloudSong[]>(JSON.parse(localStorage.getItem('qn_cloud_queue') || '[]'));
const cloudQueueIndex = ref(-1);
let cloudAudio: HTMLAudioElement | null = null;

const songId = (song: CloudSong) => String(song.id || '');

const saveCloudQueue = () => {
    localStorage.setItem('qn_cloud_queue', JSON.stringify(cloudQueue.value.slice(0, 150)));
};

const hydrateCloudNowPlaying = async (song: CloudSong, playing: boolean) => {
    currentSongName.value = song.name || t('noSongPlaying');
    currentArtistName.value = song.artist || t('unknownArtist');
    currentBaseInfo.value = song.artist ? `${song.name} - ${song.artist}` : song.name;
    setSafeTrackInfo(currentBaseInfo.value);
    coverUrl.value = song.cover || '';
    isPlaying.value = playing;
    isMediaActive.value = true;

    try {
        const lyric = await invoke<{ lyric?: string }>('get_netease_lyric', {
            id: songId(song)
        });
        parsedLyrics.value = lyric.lyric ? parseLrc(lyric.lyric) : [];
        currentMatchedIndex = -1;
        lyricQueue.value = [];
    } catch (_) {
        parsedLyrics.value = [];
    }
};

const ensureCloudAudio = () => {
    if (cloudAudio) return cloudAudio;
    cloudAudio = new Audio();
    cloudAudio.preload = 'auto';
    cloudAudio.addEventListener('ended', () => {
        playNextCloudTrack();
    });
    cloudAudio.addEventListener('play', () => {
        isPlaying.value = true;
    });
    cloudAudio.addEventListener('pause', () => {
        isPlaying.value = false;
    });
    return cloudAudio;
};

const playCloudQueueIndex = async (index: number) => {
    const song = cloudQueue.value[index];
    if (!song) return;

    neteaseStatus.value = 'Resolving playable stream...';

    try {
        const info = await invoke<CloudSongUrl>('get_netease_song_url', {
            id: songId(song),
            quality: 'hires'
        });
        if (!info.url) {
            neteaseStatus.value = info.message || 'This track is restricted by NetEase.';
            showToast(neteaseStatus.value, 'sys');
            return;
        }

        const audio = ensureCloudAudio();
        cloudQueueIndex.value = index;
        audio.src = info.url;
        await hydrateCloudNowPlaying(song, false);
        await audio.play();
        isPlaying.value = true;
        localPositionMs.value = 0;
        neteaseStatus.value = info.trial ? 'Trial stream loaded.' : `Playing ${info.quality || 'stream'}.`;
    } catch (err: any) {
        neteaseStatus.value = String(err);
        showToast('NetEase stream is not available', 'sys');
    }
};

const playNextCloudTrack = () => {
    if (!cloudQueue.value.length) return;
    const next = cloudQueueIndex.value < 0 ? 0 : (cloudQueueIndex.value + 1) % cloudQueue.value.length;
    playCloudQueueIndex(next);
};

const playPrevCloudTrack = () => {
    if (!cloudQueue.value.length) return;
    const prev = cloudQueueIndex.value <= 0 ? cloudQueue.value.length - 1 : cloudQueueIndex.value - 1;
    playCloudQueueIndex(prev);
};

const searchNetease = async () => {
    const keywords = neteaseSearchQuery.value.trim();
    if (!keywords) return;
    neteaseStatus.value = 'Searching...';
    try {
        searchResults.value = await invoke<CloudSong[]>('search_netease_songs', {
            keywords,
            limit: 12
        });
        neteaseStatus.value = searchResults.value.length ? '' : 'No songs found.';
    } catch (err: any) {
        searchResults.value = [];
        neteaseStatus.value = 'NetEase search is unavailable. Check network and try again.';
        console.error(err);
    }
};

const addSongToQueue = (song: CloudSong) => {
    const id = songId(song);
    if (!id) return;
    if (!cloudQueue.value.some(item => songId(item) === id)) {
        cloudQueue.value.push(song);
        saveCloudQueue();
    }
    islandPanelActiveTab.value = 'queue';
    if (cloudQueueIndex.value === -1) {
        playCloudQueueIndex(0);
    }
};

const loadRandomQueue = async (count: 50 | 100) => {
    neteaseStatus.value = `Loading ${count} random tracks...`;
    try {
        const songs = await invoke<CloudSong[]>('random_netease_queue', {
            count
        });
        cloudQueue.value = songs;
        cloudQueueIndex.value = -1;
        saveCloudQueue();
        islandPanelActiveTab.value = 'queue';
        neteaseStatus.value = songs.length ? `Loaded ${songs.length} tracks.` : 'No random tracks returned.';
    } catch (err: any) {
        neteaseStatus.value = 'NetEase random loading is unavailable. Check network and try again.';
        console.error(err);
    }
};

const clearCloudQueue = () => {
    cloudQueue.value = [];
    cloudQueueIndex.value = -1;
    saveCloudQueue();
};

const syncQuickSettings = async () => {
    localStorage.setItem('nsd_music_ctrl', String(isMusicCtlEnabled.value));
    localStorage.setItem('nsd_glow_border', String(isGlowBorderEnabled.value));
    localStorage.setItem('nsd_msg_notify', String(isMsgNotifyEnabled.value));
    localStorage.setItem('nsd_msg_mode', String(isMsgModeEnabled.value));
    localStorage.setItem('nsd_autohide_fs', String(isAutoHideEnabled.value));
    localStorage.setItem('nsd_pin_taskbar', String(isPinnedToTaskbar.value));
    await emit('control-music-ctl', { enabled: isMusicCtlEnabled.value });
    await emit('control-msg-mode', { enabled: isMsgModeEnabled.value });
    await emit('control-autohide-fs', { enabled: isAutoHideEnabled.value });
    await emit('control-pin-taskbar', { enabled: isPinnedToTaskbar.value });
};

const syncIslandOpacityQuick = async () => {
    localStorage.setItem('nsd_island_opacity', String(islandOpacity.value));
    await emit('control-island-opacity', { opacity: islandOpacity.value });
};

const setIslandThemeQuick = async (theme: string) => {
    islandTheme.value = theme;
    localStorage.setItem('nsd_island_theme', theme);
    await emit('control-island-theme', { theme });
};

const setSpringStyleQuick = (style: 'stiff' | 'bouncy') => {
    nsdSpringStyle.value = style;
    localStorage.setItem('nsd_spring_style', style);
};


const snapToBottomLeft = async () => {
    try {
        const appWindow = getCurrentWindow();
        await new Promise((resolve) => setTimeout(resolve, 150));
        const monitor = await currentMonitor();

        if (monitor) {
            const scaleFactor = window.devicePixelRatio;

            const WINDOW_INIT_WIDTH = currentWidth.value;
            const WINDOW_INIT_HEIGHT = currentHeight.value;
            await appWindow.setSize(new PhysicalSize(Math.ceil(WINDOW_INIT_WIDTH * scaleFactor), Math.ceil(WINDOW_INIT_HEIGHT * scaleFactor)));

            const monitorLeftPhysical = monitor.position.x;
            const monitorTopPhysical = monitor.position.y;
            
            const monitorHeightPhysical = monitor.size.height;

            
            const x = monitorLeftPhysical + (10 * scaleFactor);
            
            const y = monitorTopPhysical + monitorHeightPhysical - ((WINDOW_INIT_HEIGHT + 3) * scaleFactor);

            
            
            await appWindow.hide();

            await appWindow.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));

            
            await appWindow.show();
        }
    } catch (error) {
        console.error('停靠左下角失败:', error);
    }
};

const togglePlay = async () => {
    if (cloudAudio) {
        if (cloudAudio.paused) {
            await cloudAudio.play();
            isPlaying.value = true;
        } else {
            cloudAudio.pause();
            isPlaying.value = false;
        }
        return;
    }

    
    isPlaying.value = !isPlaying.value;

    
    try {
        await invoke('control_system_media', { action: 'play_pause' });
    } catch (err) {
        console.error('播放控制失败:', err);
        
        isPlaying.value = !isPlaying.value;
    }
};

const prevTrack = async () => {
    if (cloudAudio && cloudQueue.value.length) {
        playPrevCloudTrack();
        return;
    }
    await invoke('control_system_media', { action: 'prev' });
};

const nextTrack = async () => {
    if (cloudAudio && cloudQueue.value.length) {
        playNextCloudTrack();
        return;
    }
    await invoke('control_system_media', { action: 'next' });
};


const syncMusicStatus = async () => {
    if (cloudAudio && cloudQueueIndex.value >= 0) {
        isMediaActive.value = true;
        isPlaying.value = !cloudAudio.paused;
        localPositionMs.value = cloudAudio.currentTime * 1000;
        return;
    }

    try {
        const res = await invoke<[string, string, boolean, number, number] | null>('fetch_netease_music_info');

        if (res) {
            const [song, artist, playing, positionMs, durationMs] = res;

            if (!isMediaActive.value) isMediaActive.value = true;
            isFirstMediaCheck = false;
            isNewlyEnabled = false;

            currentSongName.value = song;
            currentArtistName.value = artist || t('unknownArtist');

            const newTrackInfo = artist ? `${song} - ${artist}` : song;

            
            if (currentBaseInfo.value !== newTrackInfo) {
                currentBaseInfo.value = newTrackInfo;
                setSafeTrackInfo(newTrackInfo);
                parsedLyrics.value = [];

                
                lyricQueue.value = [];
                currentMatchedIndex = -1;

                
                
                
                
                lastLyricChangeTime = performance.now() + 2000;

                
                localPositionMs.value = positionMs;

                if (coverCache.has(newTrackInfo)) {
                    coverUrl.value = coverCache.get(newTrackInfo)!;
                } else {
                    invoke<string>('get_random_cover_url', { songName: song, artistName: artist })
                        .then(url => {
                            coverUrl.value = url;
                            if (coverCache.size > 50) coverCache.clear();
                            coverCache.set(newTrackInfo, url);
                        }).catch(() => { coverUrl.value = ''; });
                }

                invoke<string>('fetch_netease_lyrics', { songName: song, artistName: artist, durationMs })
                    .then(lrc => {
                        if (lrc) parsedLyrics.value = parseLrc(lrc);
                    }).catch(() => { console.log('未找到歌词'); });
            } else {
                
                
                
                if (positionMs > 1000 && Math.abs(positionMs - localPositionMs.value) > 800) {
                    localPositionMs.value = positionMs - 250;
                }
            }

            isPlaying.value = playing;

            if (parsedLyrics.value.length === 0 && currentTrackInfo.value !== currentBaseInfo.value) {
                setSafeTrackInfo(currentBaseInfo.value);
            }
        } else {
            setSafeTrackInfo(`${t('noSongPlaying')} - ${getPlayerName()}`);
            isPlaying.value = false;
            coverUrl.value = '';

            if (isMediaActive.value) {
                isMediaActive.value = false;

                if (isNewlyEnabled) {
                    showToast('已开启媒体控制，暂无音频播放', 'sys');
                    isNewlyEnabled = false;
                } else if (!isFirstMediaCheck && isMusicCtlEnabled.value) {
                    showToast('无媒体活动，已切换为网速显示', 'sys');
                }
            }
            isFirstMediaCheck = false;
        }
    } catch (err) {
        console.error('音乐信息获取失败:', err);
    }
};

const showInfo = ref(false);

const getPlayerName = () => {
    const key = localStorage.getItem('nsd_target_player') || 'netease';
    const map: Record<string, string> = {
        'netease': t('neteaseMusic'),
        'spotify': 'Spotify',
        'apple': 'Apple Music',
        'qqmusic': t('qqMusicFull'),
        'kugou': t('kugouMusicFull'),
        'echo': 'Echo Music',
        'lx-music': t('lxMusicFull'),
        'other': t('genericMediaFull')
    };
    return map[key] || t('unknownPlatform');
};


const musicBoxKey = ref(0);


const currentSongName = ref(t('noSongPlaying'));
const currentArtistName = ref(getPlayerName());
const currentTrackInfo = ref(`${t('noSongPlaying')} - ${getPlayerName()}`);

watch(currentLanguage, () => {
    if (!displayMusic.value || currentSongName.value === t('noSongPlaying')) {
        currentSongName.value = t('noSongPlaying');
        currentArtistName.value = getPlayerName();
        currentTrackInfo.value = `${t('noSongPlaying')} - ${getPlayerName()}`;
    }
});


const renderQueue: string[] = [];
let isRendering = false;

const setSafeTrackInfo = (text: string) => {
    
    if (!text || text.replace(/[\s\u200B-\u200D\uFEFF\u3000]/g, '').length === 0) return;

    
    if (text === currentTrackInfo.value && renderQueue.length === 0) return;
    if (renderQueue.length > 0 && renderQueue[renderQueue.length - 1] === text) return;

    
    renderQueue.push(text);
    drainRenderQueue();
};

const drainRenderQueue = () => {
    
    if (isRendering || renderQueue.length === 0) return;

    const nextText = renderQueue.shift();
    if (!nextText || nextText === currentTrackInfo.value) {
        drainRenderQueue(); 
        return;
    }

    
    isRendering = true;
    currentTrackInfo.value = nextText;

    
    
    setTimeout(() => {
        isRendering = false;
        drainRenderQueue();
    }, 350);
};


const maskBoxRef = ref<HTMLElement | null>(null);
const textInnerRef = ref<HTMLElement | null>(null);
const scrollDist = ref(0);
const scrollDuration = ref('0s');


const calculateScroll = () => {
    if (!textInnerRef.value || !maskBoxRef.value) return;

    
    if (isMusicExpanded.value) {
        scrollDist.value = 0;
        return;
    }

    const textWidth = textInnerRef.value.getBoundingClientRect().width;
    const containerWidth = maskBoxRef.value.clientWidth;

    
    
    const safeWidth = containerWidth * 0.75;

    
    if (textWidth > safeWidth) {
        
        
        scrollDist.value = Math.ceil(textWidth - safeWidth + 5);

        
        const timeToMove = scrollDist.value / 30;

        
        const totalDuration = timeToMove / 0.6;

        scrollDuration.value = `${Math.max(totalDuration, 4.5)}s`;
    } else {
        scrollDist.value = 0;
    }
};


watch([currentTrackInfo, displayMusic, isMusicExpanded], async () => {
    await nextTick();
    setTimeout(() => {
        if (displayMusic.value) {
            calculateScroll();
        } else {
            
            scrollDist.value = 0;
        }
    }, 100);
});

let lastRx = 0;
let lastTx = 0;
let speedTimer: number;
let pingTimer: number;
let musicTimer: number;
let notifyTimer: number;


let lowTrafficStartTime = Date.now();
const RED_DELAY_MS = 5000;

const formatSpeed = (bytes: number) => {
    if (bytes < 1024) return bytes + ' B/s';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB/s';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB/s';
};

const formatBytesValue = (bytes: number) => {
    if (bytes === 0) return '0';
    const k = 1024;
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return (bytes / Math.pow(k, i)).toFixed(2);
};

const formatBytesUnit = (bytes: number) => {
    if (bytes === 0) return 'B';
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return sizes[Math.min(i, sizes.length - 1)];
};

const getLocalYYYYMMDD = (date: Date) => {
    const y = date.getFullYear();
    const m = String(date.getMonth() + 1).padStart(2, '0');
    const d = String(date.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
};

const totalUpload = computed(() => Object.values(trafficData.value).reduce((acc, curr) => acc + curr.up, 0));
const totalDownload = computed(() => Object.values(trafficData.value).reduce((acc, curr) => acc + curr.down, 0));
const monthTraffic = computed(() => {
    const currentMonth = getLocalYYYYMMDD(new Date()).slice(0, 7);
    return Object.entries(trafficData.value)
        .filter(([date]) => date.startsWith(currentMonth))
        .reduce((acc, [, data]) => acc + data.up + data.down, 0);
});

const loadTrafficData = () => {
    try {
        const stored = localStorage.getItem('nsd_traffic_stats');
        if (stored) trafficData.value = JSON.parse(stored);
    } catch (e) {
        console.error('加载统计数据失败', e);
    }
};

loadTrafficData();

let saveThrottleCounter = 0;


const fetchSpeedStats = async () => {
    try {
        const [currentRx, currentTx] = await invoke<[number, number]>('get_network_stats');
        if (lastRx !== 0) {
            const rxDiff = currentRx - lastRx;
            const txDiff = currentTx - lastTx;

            downloadSpeed.value = formatSpeed(rxDiff);
            uploadSpeed.value = formatSpeed(txDiff);

            if (rxDiff > 0 || txDiff > 0) {
                const todayStr = getLocalYYYYMMDD(new Date());
                if (!trafficData.value[todayStr]) {
                    trafficData.value[todayStr] = { up: 0, down: 0 };
                }
                trafficData.value[todayStr].down += rxDiff;
                trafficData.value[todayStr].up += txDiff;

                saveThrottleCounter++;
                if (saveThrottleCounter >= 5) {
                    localStorage.setItem('nsd_traffic_stats', JSON.stringify(trafficData.value));
                    saveThrottleCounter = 0;
                }
            }

            
            const limit = 1024 * 1024;
            const currentDownloadHigh = rxDiff >= limit;
            const currentUploadHigh = txDiff >= limit;

            isHighDownload.value = currentDownloadHigh;
            isHighUpload.value = currentUploadHigh;

            
            if (currentDownloadHigh || currentUploadHigh) {
                
                lowTrafficStartTime = Date.now();
            }
        }
        lastRx = currentRx;
        lastTx = currentTx;
    } catch (error) {
        console.error('流量获取失败:', error);
    }
};


const checkNetworkLatency = async () => {
    try {
        const latency = await invoke<number>('get_network_latency');

        
        if (latency < 150) {
            networkStatus.value = 'good';      
        } else {
            networkStatus.value = 'warning';   
        }
    } catch (error) {
        

        
        if (isHighDownload.value || isHighUpload.value) {
            networkStatus.value = 'warning';
            return;
        }

        
        const timeSinceLowTraffic = Date.now() - lowTrafficStartTime;
        if (timeSinceLowTraffic < RED_DELAY_MS) {
            
            networkStatus.value = 'warning';
        } else {
            
            networkStatus.value = 'error';
        }
    }
};


watch(networkStatus, (newStatus, oldStatus) => {
    
    if (oldStatus && oldStatus !== newStatus) {
        if (newStatus === 'error') {
            showToast(t('networkDisconnected'), 'sys');
        } else if (newStatus === 'good' && oldStatus === 'error') {
            showToast(t('networkRestored'), 'sys');
        }
    }
});


const adjustWindowPosition = async () => {
    try {
        const appWindow = getCurrentWindow();
        await new Promise((resolve) => setTimeout(resolve, 150));
        const monitor = await currentMonitor();

        if (monitor) {
            const scaleFactor = window.devicePixelRatio;

            const WINDOW_INIT_WIDTH = currentWidth.value;   
            const WINDOW_INIT_HEIGHT = currentHeight.value; 
            await appWindow.setSize(new PhysicalSize(Math.ceil(WINDOW_INIT_WIDTH * scaleFactor), Math.ceil(WINDOW_INIT_HEIGHT * scaleFactor)));

            const monitorWidthPhysical = monitor.size.width;
            const monitorLeftPhysical = monitor.position.x;
            const monitorTopPhysical = monitor.position.y;

            
            const windowSize = await appWindow.innerSize();
            const windowWidthPhysical = windowSize.width;

            const x = monitorLeftPhysical + (monitorWidthPhysical - windowWidthPhysical) / 2;
            const y = monitorTopPhysical + (12 * scaleFactor);

            await appWindow.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
        }
    } catch (error) {
        console.error('调整窗口位置失败:', error);
    } finally {
        try {
            await getCurrentWindow().show();
        } catch (e) {
            console.error(e);
        }
    }
};

const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

const easeOutCubic = (t: number) => 1 - Math.pow(1 - Math.min(Math.max(t, 0), 1), 3);

const uiSpringProgress = (elapsedSeconds: number, response = 0.32, dampingRatio = 1) => {
    const omega = 2 * Math.PI / Math.max(response, 0.001);

    if (dampingRatio >= 1) {
        return 1 - Math.exp(-omega * elapsedSeconds) * (1 + omega * elapsedSeconds);
    }

    const damped = omega * Math.sqrt(1 - dampingRatio * dampingRatio);
    return 1 - Math.exp(-dampingRatio * omega * elapsedSeconds) *
        (Math.cos(damped * elapsedSeconds) +
            (dampingRatio / Math.sqrt(1 - dampingRatio * dampingRatio)) * Math.sin(damped * elapsedSeconds));
};

const onEnter = (el: Element, done: () => void) => {
    const HTMLElement = el as HTMLElement;
    HTMLElement.style.transformOrigin = 'center top';
    let start = performance.now();

    const duration = prefersReducedMotion ? 160 : 360;
    HTMLElement.style.transform = prefersReducedMotion ? 'none' : 'scale(0.96)';
    HTMLElement.style.opacity = '0';

    const animate = (time: number) => {
        let t = (time - start) / 1000;
        let progress = (time - start) / duration;

        let scale = prefersReducedMotion ? 1 : 0.96 + 0.04 * uiSpringProgress(t, 0.32, 1);
        let opacity = easeOutCubic(progress);

        HTMLElement.style.transform = prefersReducedMotion ? 'none' : `scale(${scale})`;
        HTMLElement.style.opacity = opacity.toString();

        if (progress < 1) {
            requestAnimationFrame(animate);
        } else {
            HTMLElement.style.transform = `scale(1)`;
            HTMLElement.style.opacity = '1';
            done();
        }
    };
    requestAnimationFrame(animate);
};

const onLeave = (el: Element, done: () => void) => {
    const HTMLElement = el as HTMLElement;
    HTMLElement.style.transformOrigin = 'center top';
    let start = performance.now();

    const duration = prefersReducedMotion ? 140 : 220;

    const animate = (time: number) => {
        let progress = (time - start) / duration;
        let eased = easeOutCubic(progress);
        let scale = prefersReducedMotion ? 1 : 1 - 0.035 * eased;
        let opacity = 1 - eased;

        HTMLElement.style.transform = prefersReducedMotion ? 'none' : `scale(${Math.max(0.965, scale)})`;
        HTMLElement.style.opacity = Math.max(0, opacity).toString();

        if (progress < 1) {
            requestAnimationFrame(animate);
        } else {
            done();
            
            getCurrentWindow().hide().catch(console.error);
        }
    };
    requestAnimationFrame(animate);
};

let mouseDownX = 0;
let mouseDownY = 0;
let isMouseDown = false;

const handleMouseDown = (event: MouseEvent) => {
    if ((event.target as HTMLElement).closest('.ctl-btn, .island-hover-panel, input, button')) return;

    
    mouseDownX = event.clientX;
    mouseDownY = event.clientY;
    isMouseDown = true;
};

const handleMouseMove = async (event: MouseEvent) => {
    if (!isMouseDown) return;

    
    if (isSizeAnimating) return;

    
    if (isMusicExpanded.value || isMusicExpanding.value || isIslandPanelOpen.value || isMsgActive.value || displaySysToast.value) {
        
        isMouseDown = false;
        return;
    }

    
    if (isPinnedToTaskbar.value || isPositionLocked.value) return;

    if (Math.abs(event.clientX - mouseDownX) > 5 || Math.abs(event.clientY - mouseDownY) > 5) {
        isMouseDown = false;
        try {
            await getCurrentWindow().startDragging();
        } catch (error) {
            console.error('拖拽失败:', error);
        }
    }
};

const handleMouseUp = () => {
    isMouseDown = false;
};

const handleRightClick = async (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation(); 

    
    const toggleGlowBorderItem = await MenuItem.new({
        text: isGlowBorderEnabled.value ? t('disableGlowBorder') : t('enableGlowBorder'),
        id: 'toggle_glow_border',
        enabled: true,
        action: () => {
            isGlowBorderEnabled.value = !isGlowBorderEnabled.value;
            localStorage.setItem('nsd_glow_border', String(isGlowBorderEnabled.value));
            showToast(isGlowBorderEnabled.value ? t('glowBorderEnabled') : t('glowBorderDisabled'));
        }
    });

    
    const resetPositionItem = await MenuItem.new({
        text: isPinnedToTaskbar.value ? t('resetPositionLocked') : t('resetPosition'),
        id: 'reset_position',
        enabled: !isPinnedToTaskbar.value,
        action: async () => {
            try {
                await adjustWindowPosition();
                showToast(t('positionReset'));
            } catch (error) {
                console.error(error);
            }
        }
    });

    
    const toggleLockItem = await MenuItem.new({
        text: isPositionLocked.value ? t('unlockCurrentLocked') : t('lock'),
        id: 'toggle_lock',
        enabled: !isPinnedToTaskbar.value,
        action: () => {
            isPositionLocked.value = !isPositionLocked.value;
            localStorage.setItem('nsd_position_locked', String(isPositionLocked.value));
            
            showToast(
                isPositionLocked.value ? t('positionLocked') : t('positionUnlocked'),
                isPositionLocked.value ? 'lock' : 'unlock'
            );
        }
    });

    
    const closeItem = await MenuItem.new({
        text: '隐藏灵动岛',
        id: 'close',
        action: () => {
            isIslandVisible.value = false;
        }
    });

    const quitItem = await MenuItem.new({
        text: '退出 QiuNiu',
        id: 'quit_app',
        action: async () => {
            await invoke('quit_app');
        }
    });

    
    const position = new LogicalPosition(
        event.clientX,
        event.clientY
    );

    
    const menu = await Menu.new();
    await menu.append(toggleGlowBorderItem);
    await menu.append(resetPositionItem);
    await menu.append(toggleLockItem);
    await menu.append(closeItem);
    await menu.append(quitItem);

    
    try {
        isMenuOpen.value = true; 
        await menu.popup(position);
    } catch (error) {
        console.error('菜单弹出失败:', error);
    } finally {
        isMenuOpen.value = false; 
    }
};

const onInnerEnter = (el: Element, done: () => void) => {
    const htmlEl = el as HTMLElement;
    let start = performance.now();

    
    const duration = 180;
    htmlEl.style.transformOrigin = 'center';
    htmlEl.style.opacity = '0';
    htmlEl.style.transform = 'none'; 

    const animate = (time: number) => {
        let progress = (time - start) / duration;
        htmlEl.style.opacity = Math.min(1, progress).toString();

        if (progress < 1) {
            requestAnimationFrame(animate);
        } else {
            htmlEl.style.opacity = '1';
            done();
        }
    };
    requestAnimationFrame(animate);
};

const onInnerLeave = (el: Element, done: () => void) => {
    const htmlEl = el as HTMLElement;
    let start = performance.now();
    const duration = 140;

    const animate = (time: number) => {
        let progress = (time - start) / duration;
        let opacity = 1 - progress;

        htmlEl.style.opacity = Math.max(0, opacity).toString();

        if (progress < 1) {
            requestAnimationFrame(animate);
        } else {
            done();
        }
    };
    requestAnimationFrame(animate);
};


let isSizeAnimating = false;
let sizeAnimTimer: number | null = null;


const appScale = ref(Number(localStorage.getItem('nsd_app_scale')) || 1.0);


watch(appScale, (newScale) => {
    (document.documentElement.style as any).zoom = newScale;
}, { immediate: true });


const animateIslandSize = async (targetWidth: number, targetHeight: number) => {
    try {
        
        const finalWidth = targetWidth * appScale.value;
        const finalHeight = targetHeight * appScale.value;

        
        isSizeAnimating = true;
        if (sizeAnimTimer) clearTimeout(sizeAnimTimer);

        const settleMs = nsdSpringStyle.value === 'bouncy' ? 640 : 480;
        sizeAnimTimer = window.setTimeout(() => {
            isSizeAnimating = false;
        }, settleMs);

        const appWindow = getCurrentWindow();
        const realSize = await appWindow.innerSize();
        const scaleFactor = window.devicePixelRatio;

        const realStartW = realSize.width / scaleFactor;
        const realStartH = realSize.height / scaleFactor;

        await invoke('start_island_animation', {
            startWidth: realStartW,
            startHeight: realStartH,
            targetWidth: finalWidth,    
            targetHeight: finalHeight,  
            isPinned: isPinnedToTaskbar.value,
            springStyle: nsdSpringStyle.value
        });
    } catch (err) {
        console.error('呼叫 Rust 动画失败:', err);
        isSizeAnimating = false;
    }
};


const collapseMusic = () => {
    const { w, h } = getBaseSize();
    const targetW = w * appScale.value;
    const targetH = h * appScale.value;
    const windowLooksExpanded =
        Math.abs(currentWidth.value - targetW) > 8 ||
        Math.abs(currentHeight.value - targetH) > 8;

    if (!isMusicExpanded.value && !isMusicExpanding.value && !isIslandPanelOpen.value && !windowLooksExpanded) return;

    isIslandPanelOpen.value = false;
    isMusicExpanded.value = false;
    isMusicExpanding.value = false;

    if (musicExpandAnimTimer) {
        clearTimeout(musicExpandAnimTimer);
        musicExpandAnimTimer = null;
    }

    animateIslandSize(w, h);
};


const expandMusic = (e: MouseEvent) => {
    if (Math.abs(e.clientX - mouseDownX) > 5 || Math.abs(e.clientY - mouseDownY) > 5) return;
    if ((e.target as HTMLElement).closest('.ctl-btn')) return;
    if (isIslandPanelOpen.value) return;

    if (isMusicExpanded.value || isMusicExpanding.value) return;

    isMusicExpanding.value = true;
    isMusicExpanded.value = true;
    animateIslandSize(nsdMusicExpandedWidth.value, 115);

    if (musicExpandAnimTimer) clearTimeout(musicExpandAnimTimer);
    musicExpandAnimTimer = window.setTimeout(() => {
        isMusicExpanding.value = false;
        musicExpandAnimTimer = null;
    }, nsdSpringStyle.value === 'bouncy' ? 520 : 360);
};

const openIslandPanel = () => {
    if (isMsgActive.value || displaySysToast.value) return;
    if (hoverCloseTimer) {
        clearTimeout(hoverCloseTimer);
        hoverCloseTimer = null;
    }
    if (isIslandPanelOpen.value) return;

    if (hoverOpenTimer) clearTimeout(hoverOpenTimer);
    hoverOpenTimer = window.setTimeout(() => {
        if (!isPointerInsideIsland.value || isMsgActive.value || displaySysToast.value) return;
        isMusicExpanded.value = false;
        isMusicExpanding.value = false;
        isIslandPanelOpen.value = true;
        animateIslandSize(Math.max(nsdMusicExpandedWidth.value, 430), 410);
    }, 120);
};

const handlePanelInputFocus = () => {
    isPanelInputFocused.value = true;
    if (hoverCloseTimer) {
        clearTimeout(hoverCloseTimer);
        hoverCloseTimer = null;
    }
};

const handlePanelInputBlur = () => {
    window.setTimeout(() => {
        isPanelInputFocused.value = false;
    }, 120);
};

const scheduleCloseIslandPanel = () => {
    if (hoverOpenTimer) {
        clearTimeout(hoverOpenTimer);
        hoverOpenTimer = null;
    }
    if (hoverCloseTimer) clearTimeout(hoverCloseTimer);
    hoverCloseTimer = window.setTimeout(() => {
        if (isPanelInputFocused.value) return;
        collapseMusic();
    }, 90);
};


const handleMouseLeave = () => {
    isPointerInsideIsland.value = false;
    scheduleCloseIslandPanel();
};


const handleMouseEnter = () => {
    isPointerInsideIsland.value = true;
    openIslandPanel();
};

watch(displayMusic, (newVal: boolean) => {
    if (!newVal) {
        collapseMusic(); 
    }
});

import defaultLogo from '../assets/qiuniu.logo';
const currentMsgIcon = ref(defaultLogo);

const getAppIcon = (_appName: string) => {
    return defaultLogo;
};

onMounted(async () => {
    const appWindow = getCurrentWindow();

    window.addEventListener('blur', collapseMusic);
    hoverWatchdogTimer = window.setInterval(() => {
        if (isPointerInsideIsland.value || isPanelInputFocused.value || isMsgActive.value || displaySysToast.value) return;
        const { w, h } = getBaseSize();
        const targetW = w * appScale.value;
        const targetH = h * appScale.value;
        const windowLooksExpanded =
            Math.abs(currentWidth.value - targetW) > 10 ||
            Math.abs(currentHeight.value - targetH) > 10;
        if (isIslandPanelOpen.value || isMusicExpanded.value || isMusicExpanding.value || windowLooksExpanded) {
            collapseMusic();
        }
    }, 650) as unknown as number;

    document.addEventListener('contextmenu', (e) => {
        e.preventDefault();
    }, { capture: true }); 

    
    await listen<{ enabled: boolean }>('control-music-ctl', (event) => {
        const isEnabled = event.payload.enabled;
        isMusicCtlEnabled.value = isEnabled;

        if (isEnabled) {
            
            if (localStorage.getItem('nsd_glow_border') === null) {
                isGlowBorderEnabled.value = true;
                localStorage.setItem('nsd_glow_border', 'true');
            }

            
            isMediaActive.value = true;
            isNewlyEnabled = true;

            showInfo.value = false;
            musicBoxKey.value++;
        } else {
            
            isMediaActive.value = true;
            isNewlyEnabled = false;
        }
    });

    
    await listen<any>('sync-dynamic-settings', async (event) => {
        const data = event.payload;
        nsdBaseWidth.value = Number(data.baseWidth);
        nsdBaseHeight.value = Number(data.baseHeight);
        nsdMusicBaseWidth.value = Number(data.musicBaseWidth) || 260;
        nsdMusicExpandedWidth.value = Number(data.musicExpandedWidth);
        nsdMsgExpandedWidth.value = Number(data.msgExpandedWidth);
        nsdBorderRadius.value = Number(data.borderRadius);
        nsdSpringStyle.value = data.springStyle;

        
        const oldScale = appScale.value;
        appScale.value = Number(data.appScale) || 1.0;

        
        if (oldScale !== appScale.value) {
            if (isMusicExpanded.value) {
                animateIslandSize(nsdMusicExpandedWidth.value, 115);
            } else if (isMsgActive.value) {
                animateIslandSize(nsdMsgExpandedWidth.value, 65);
            } else {
                const { w, h } = getBaseSize();
                animateIslandSize(w, h);
            }
        }

        
        const newAlwaysOnTop = data.isAlwaysOnTop !== false;
        const appWindow = getCurrentWindow();

        
        if (isAlwaysOnTop.value !== newAlwaysOnTop) {
            isAlwaysOnTop.value = newAlwaysOnTop;
            await appWindow.setAlwaysOnTop(newAlwaysOnTop);
        }

        
        if (!isMsgActive.value && !displaySysToast.value && !isMusicExpanded.value && !isMusicExpanding.value) {
            const { w, h } = getBaseSize();
            animateIslandSize(w, h);
        }
    });

    
    await listen<string>('system-event', (event) => {
        let text = event.payload;
        const volumeMatch = text.match(/当前系统音量 (\d+)%/);
        if (volumeMatch) {
            text = t('systemVolume', { volume: volumeMatch[1] });
        } else if (text === '正在使用电池供电') {
            text = t('batteryPowered');
        }
        showToast(text, 'sys');
    });

    
    await listen<{ state: 'charging' | 'discharging', percent: number }>('battery-event', (event) => {
        const { state, percent } = event.payload;

        if (state === 'charging') {
            showToast(t('powerPlugged', { percent }), 'battery-charge');
        } else if (state === 'discharging' && percent <= 20) {
            
            showToast(t('batteryLow', { percent }), 'battery-low');
        }
    });

    
    await listen<{ opacity: number }>('control-island-opacity', (event) => {
        islandOpacity.value = event.payload.opacity;
    });

    
    await listen<{ theme: string }>('control-island-theme', (event) => {
        islandTheme.value = event.payload.theme;
    });

    
    await listen<{ enabled: boolean }>('control-pin-taskbar', async (event) => {
        isPinnedToTaskbar.value = event.payload.enabled;
        if (isPinnedToTaskbar.value) {
            await snapToBottomLeft(); 
        } else {
            await adjustWindowPosition(); 
        }
    });

    
    await listen<{ enabled: boolean }>('control-msg-mode', async (event) => {
        isMsgModeEnabled.value = event.payload.enabled;
        if (isMsgModeEnabled.value && !isMsgActive.value) {
            
            isIslandVisible.value = false;
        } else if (!isMsgModeEnabled.value) {
            
            await getCurrentWindow().show();
            isIslandVisible.value = true;
        }
    });

    await listen<{ language: AppLanguage }>('control-language', (event) => {
        currentLanguage.value = event.payload.language;
    });

    
    await listen<{ enabled: boolean }>('control-autohide-fs', (event) => {
        isAutoHideEnabled.value = event.payload.enabled;
    });

    
    await listen<boolean>('fullscreen-changed', async (event) => {
        const isFullscreen = event.payload;

        
        if (!isAutoHideEnabled.value) return;

        if (isFullscreen) {
            
            if (isIslandVisible.value) {
                wasVisibleBeforeFullscreen = true;
                isIslandVisible.value = false; 
            }
        } else {
            
            if (wasVisibleBeforeFullscreen) {
                await getCurrentWindow().show();

                
                setTimeout(() => {
                    isIslandVisible.value = true;
                }, 40);

                wasVisibleBeforeFullscreen = false; 
            }
        }
    });

    try {
        await appWindow.innerPosition();
    } catch (e) { }

    
    const { w, h } = getBaseSize();
    currentWidth.value = w * appScale.value;
    currentHeight.value = h * appScale.value;

    
    if (isPinnedToTaskbar.value) {
        await snapToBottomLeft();
    } else {
        await adjustWindowPosition();
    }

    
    
    if (!isMsgModeEnabled.value) {
        await getCurrentWindow().show();
        isIslandVisible.value = true;
    }

    fetchSpeedStats();
    checkNetworkLatency();

    
    speedCycleTimer = window.setInterval(() => {
        
        if (displaySpeed.value) {
            isShowingUpload.value = !isShowingUpload.value;
        }
    }, 5000);

    
    
    speedTimer = setInterval(async () => {
        
        if (isAlwaysOnTop.value && isPinnedToTaskbar.value && isIslandVisible.value && !isMenuOpen.value) {
            invoke('force_window_topmost').catch(() => { });
        }

        
        fetchSpeedStats();
    }, 800) as unknown as number;


    
    musicTimer = setInterval(() => {
        if (isMusicCtlEnabled.value) {
            syncMusicStatus();
        }
    }, 2000);


    
    notifyTimer = setInterval(async () => {
        const enabled = localStorage.getItem('nsd_msg_notify') === 'true';
        if (!enabled) return;

        try {
            const res = await invoke<any>('fetch_latest_notification');
            if (res) {
                msgAumid.value = res.aumid;

                
                msgTitle.value = (res.title && res.title !== res.app_name) ? res.title : t('newNotification');
                
                msgAppName.value = res.app_name;
                
                msgBody.value = res.body || (res.title === res.app_name ? t('receivedNotification') : res.title);

                currentMsgIcon.value = getAppIcon(res.app_name);

                if (!isMsgActive.value) {
                    isMsgActive.value = true;
                    if (isMsgModeEnabled.value && !isIslandVisible.value) {
                        getCurrentWindow().show();
                        isIslandVisible.value = true;
                    }
                    if (!isPinnedToTaskbar.value) {
                        animateIslandSize(nsdMsgExpandedWidth.value, 65);
                    }
                }

                if ((window as any).msgTimer) clearTimeout((window as any).msgTimer);
                (window as any).msgTimer = setTimeout(() => {
                    isMsgActive.value = false;
                    const { w, h } = getBaseSize();
                    animateIslandSize(w, h);
                    if (isMsgModeEnabled.value) {
                        setTimeout(() => {
                            if (!isMsgActive.value) isIslandVisible.value = false;
                        }, 600);
                    }
                }, 5000);
            }
        } catch (err) {
            console.error(err);
        }
    }, 2500);

    
    pingTimer = setInterval(checkNetworkLatency, 5500) as unknown as number;

    
    await listen<{ show: boolean }>('control-island-visibility', async (event) => {
        if (event.payload.show) {
            
            await getCurrentWindow().show();
            await getCurrentWindow().setAlwaysOnTop(true);
            
            setTimeout(() => {
                isIslandVisible.value = true;
            }, 40);
        } else {
            
            isIslandVisible.value = false;
        }
    });

    
    await listen<number[]>("island-resize", (event) => {
        const [w, h] = event.payload;
        currentWidth.value = w;
        currentHeight.value = h;
    });

    
    spectrumTimer = setInterval(async () => {
        
        const now = performance.now();
        const delta = now - lastTickTime;
        lastTickTime = now;

        if (isPlaying.value) {
            
            localPositionMs.value += delta;

            
            if (parsedLyrics.value.length > 0) {
                let matchedIndex = -1;

                
                for (let i = 0; i < parsedLyrics.value.length; i++) {
                    
                    if (parsedLyrics.value[i].time <= localPositionMs.value + 550) {
                        matchedIndex = i;
                    } else {
                        break;
                    }
                }

                
                if (matchedIndex > currentMatchedIndex) {
                    
                    if (currentMatchedIndex === -1) {
                        lyricQueue.value = [];
                        lyricQueue.value.push(parsedLyrics.value[matchedIndex].text);
                    }
                    
                    else if (matchedIndex - currentMatchedIndex > 2) {
                        lyricQueue.value = [];
                        lyricQueue.value.push(parsedLyrics.value[matchedIndex].text);
                    }
                    
                    else {
                        for (let i = currentMatchedIndex + 1; i <= matchedIndex; i++) {
                            lyricQueue.value.push(parsedLyrics.value[i].text);
                        }
                    }
                    currentMatchedIndex = matchedIndex;
                } else if (matchedIndex < currentMatchedIndex && matchedIndex !== -1) {
                    
                    lyricQueue.value = [];
                    lyricQueue.value.push(parsedLyrics.value[matchedIndex].text);
                    currentMatchedIndex = matchedIndex;
                }

                
                if (lyricQueue.value.length > 0) {
                    const now = performance.now();
                    
                    if (now - lastLyricChangeTime >= 800) {
                        const nextLyric = lyricQueue.value.shift();
                        if (nextLyric && nextLyric !== currentTrackInfo.value) {
                            setSafeTrackInfo(nextLyric);
                            lastLyricChangeTime = now;
                        }
                    }
                }
            }

            
            if (showSpectrumIndicator.value) {
                try {
                    const data = await invoke<number[]>('get_audio_spectrum');
                    spectrumData.value = data;
                } catch (err) {
                    
                }
            }
        } else {
            
            spectrumData.value = [0.35, 0.35, 0.35, 0.35, 0.35];
        }
    }, 50) as unknown as number;

    
    setTimeout(() => {
        calculateScroll();
    }, 700);
});

onUnmounted(() => {
    window.removeEventListener('blur', collapseMusic);
    clearInterval(speedTimer);
    clearInterval(pingTimer);
    clearInterval(musicTimer);
    clearInterval(notifyTimer);
    clearInterval(spectrumTimer);
    if (hoverOpenTimer) clearTimeout(hoverOpenTimer);
    if (hoverCloseTimer) clearTimeout(hoverCloseTimer);
    if (hoverWatchdogTimer) clearInterval(hoverWatchdogTimer);
    if (speedCycleTimer) clearInterval(speedCycleTimer);
    localStorage.setItem('nsd_traffic_stats', JSON.stringify(trafficData.value));
    if (cloudAudio) {
        cloudAudio.pause();
        cloudAudio.src = '';
        cloudAudio = null;
    }
});
</script>

<style scoped>
*,
*::before,
*::after {
    box-sizing: border-box;
    border: none !important;
    outline: none !important;
}

:root {
    -webkit-app-region: drag;
}

:global(html),
:global(body) {
    background-color: transparent !important;
    background: transparent !important;
    overflow: hidden;
    margin: 0;
    padding: 0;
    border: none !important;
    width: 100%;   
    height: 100%;  
}

:global(#app) {
    width: 100%;
    height: 100%;
}


.island-container {
    
    margin: 0 auto;
    
    border-radius: 100px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2px;
    user-select: none;
    -webkit-user-select: none;
    overflow: hidden;
    background: transparent;
    transition: background-color 0.22s ease, color 0.22s ease;
    box-sizing: border-box;
    transform: translateZ(0);
    will-change: transform, opacity;
    contain: strict;
}


.rainbow-border-glow {
    position: absolute;
    width: 500px;
    height: 500px;

    
    top: calc(50% - 250px);
    left: calc(50% - 250px);

    z-index: 1;

    
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='500' height='500'%3E%3Cdefs%3E%3Cfilter id='b' x='-50%25' y='-50%25' width='200%25' height='200%25'%3E%3CfeGaussianBlur in='SourceGraphic' stdDeviation='60'/%3E%3C/filter%3E%3C/defs%3E%3Cg filter='url(%23b)'%3E%3Ccircle cx='250' cy='90' r='150' fill='%23ff3b30'/%3E%3Ccircle cx='390' cy='170' r='150' fill='%23ff9500'/%3E%3Ccircle cx='390' cy='330' r='150' fill='%234cd964'/%3E%3Ccircle cx='250' cy='410' r='150' fill='%23007aff'/%3E%3Ccircle cx='110' cy='330' r='150' fill='%235856d6'/%3E%3Ccircle cx='110' cy='170' r='150' fill='%23ff2d55'/%3E%3C/g%3E%3C/svg%3E");
    background-size: cover;

    
    animation: rainbow-rotate 10s linear infinite;
    will-change: transform;
}


.island-core-content {
    position: relative;
    z-index: 2;
    width: 100%;
    height: 100%;
    border-radius: 98px;
    transform: translateZ(0);
    backface-visibility: hidden;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 14px;
    overflow: hidden;
}

.island-core-content.panel-open {
    flex-direction: column;
    align-items: stretch;
    justify-content: flex-start;
    gap: 10px;
    padding: 12px;
}

.island-core-content.panel-open .inner-wrapper {
    flex: 0 0 48px;
    width: 100%;
    height: 48px;
}

.island-core-content.panel-open .audio-spectrum,
.island-core-content.panel-open .status-dot {
    position: absolute;
    right: 16px;
    top: 26px;
}

.island-hover-panel {
    width: 100%;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    color: inherit;
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
    -webkit-app-region: no-drag;
}

.panel-tabs {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
    padding: 4px;
    border-radius: 10px;
    background: rgba(142, 142, 147, 0.16);
}

.panel-tab,
.panel-icon-btn,
.panel-pill-btn,
.song-row,
.segmented button {
    color: inherit;
    cursor: pointer;
    -webkit-app-region: no-drag;
    font-family: inherit;
}

.panel-tab {
    height: 28px;
    border-radius: 7px;
    background: transparent;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background-color 0.16s ease, transform 0.08s ease-out;
}

.panel-tab svg {
    width: 15px;
    height: 15px;
}

.panel-tab.active {
    background: rgba(255, 255, 255, 0.18);
}

.panel-tab:active,
.panel-icon-btn:active,
.panel-pill-btn:active,
.song-row:active,
.segmented button:active {
    transform: scale(0.97);
}

.panel-section {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    gap: 8px;
}

.settings-section {
    overflow-y: auto;
    padding-right: 2px;
}

.search-row {
    display: grid;
    grid-template-columns: 1fr 34px;
    gap: 8px;
}

.panel-search-input,
.api-field input {
    height: 34px;
    border-radius: 8px;
    padding: 0 10px;
    background: rgba(142, 142, 147, 0.18);
    color: inherit;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0;
    -webkit-app-region: no-drag;
}

.mini-range {
    width: 150px;
    accent-color: #34C759;
    -webkit-app-region: no-drag;
}

.panel-search-input::placeholder {
    color: currentColor;
    opacity: 0.42;
}

.panel-icon-btn {
    width: 34px;
    height: 34px;
    border-radius: 8px;
    background: rgba(48, 209, 88, 0.22);
    display: flex;
    align-items: center;
    justify-content: center;
}

.panel-icon-btn svg {
    width: 15px;
    height: 15px;
}

.random-row,
.queue-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
}

.queue-toolbar {
    font-size: 12px;
    font-weight: 700;
    opacity: 0.78;
}

.panel-pill-btn {
    height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    background: rgba(142, 142, 147, 0.18);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0;
    transition: background-color 0.16s ease, transform 0.08s ease-out;
}

.panel-pill-btn:hover,
.song-row:hover,
.segmented button:hover {
    background: rgba(255, 255, 255, 0.16);
}

.panel-list {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-right: 2px;
}

.panel-list::-webkit-scrollbar {
    width: 4px;
}

.panel-list::-webkit-scrollbar-thumb {
    background: rgba(142, 142, 147, 0.35);
    border-radius: 4px;
}

.song-row {
    width: 100%;
    min-height: 42px;
    display: grid;
    grid-template-columns: 32px 1fr 20px;
    align-items: center;
    gap: 9px;
    padding: 5px 7px;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    transition: background-color 0.16s ease, transform 0.08s ease-out;
}

.song-row.active {
    background: rgba(48, 209, 88, 0.2);
}

.song-thumb {
    width: 32px;
    height: 32px;
    border-radius: 6px;
    object-fit: cover;
    background: linear-gradient(135deg, #1f4037 0%, #d7b66a 100%);
}

.song-meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.song-name,
.song-sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    letter-spacing: 0;
}

.song-name {
    font-size: 12.5px;
    font-weight: 700;
}

.song-sub {
    font-size: 11px;
    font-weight: 600;
    opacity: 0.58;
}

.song-action {
    font-size: 18px;
    font-weight: 600;
    opacity: 0.7;
    text-align: center;
}

.panel-empty {
    margin: auto;
    max-width: 280px;
    text-align: center;
    font-size: 12px;
    line-height: 1.35;
    opacity: 0.58;
}

.status-line {
    margin-top: 6px;
}

.setting-line,
.api-field {
    min-height: 42px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 2px;
    font-size: 12.5px;
    font-weight: 700;
}

.stats-section {
    overflow: hidden;
}

.live-speed-grid,
.stat-mini-grid {
    display: grid;
    gap: 8px;
}

.live-speed-grid {
    grid-template-columns: 1fr 1fr;
}

.live-speed-card,
.stat-mini-grid > div,
.network-row {
    border-radius: 9px;
    background: rgba(142, 142, 147, 0.16);
}

.live-speed-card {
    min-height: 58px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
}

.live-speed-card span,
.stat-mini-grid span {
    font-size: 11px;
    font-weight: 700;
    opacity: 0.55;
}

.live-speed-card strong {
    font-size: 17px;
    line-height: 1.1;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0;
}

.stat-mini-grid {
    grid-template-columns: 1fr;
}

.stat-mini-grid > div {
    min-height: 44px;
    padding: 8px 10px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
}

.stat-mini-grid strong {
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0;
    white-space: nowrap;
}

.network-row {
    height: 38px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    font-size: 12px;
    font-weight: 700;
}

.network-dot-large {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
}

.api-field {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
}

.api-field span {
    opacity: 0.72;
}

.panel-switch {
    position: relative;
    width: 42px;
    height: 24px;
    flex-shrink: 0;
    -webkit-app-region: no-drag;
}

.panel-switch input {
    opacity: 0;
    width: 0;
    height: 0;
}

.panel-switch span {
    position: absolute;
    inset: 0;
    border-radius: 999px;
    background: rgba(142, 142, 147, 0.28);
    transition: background-color 0.18s ease;
}

.panel-switch span::before {
    content: "";
    position: absolute;
    width: 18px;
    height: 18px;
    left: 3px;
    top: 3px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.22);
    transition: transform 0.22s cubic-bezier(0.2, 0, 0, 1);
}

.panel-switch input:checked + span {
    background: #34C759;
}

.panel-switch input:checked + span::before {
    transform: translateX(18px);
}

.segmented {
    display: flex;
    gap: 4px;
    padding: 3px;
    border-radius: 9px;
    background: rgba(142, 142, 147, 0.16);
}

.segmented button {
    min-width: 48px;
    height: 26px;
    border-radius: 7px;
    padding: 0 8px;
    background: transparent;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0;
}

.segmented button.active {
    background: rgba(255, 255, 255, 0.18);
}

.panel-material-enter-active,
.panel-material-leave-active {
    transition: opacity 0.18s ease, transform 0.22s cubic-bezier(0.2, 0, 0, 1), filter 0.2s ease;
    transform-origin: top center;
}

.panel-material-enter-from,
.panel-material-leave-to {
    opacity: 0;
    transform: translateY(-6px) scale(0.98);
    filter: blur(6px);
}


@keyframes rainbow-rotate {
    from {
        transform: rotate(0deg);
    }

    to {
        transform: rotate(360deg);
    }
}

[data-tauri-drag-region] {
    -webkit-app-region: drag;
    cursor: grab;
}

[data-tauri-drag-region]:active {
    cursor: grabbing;
}


.speed-box {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    width: 100%;
    height: 100%;
}

.speed-item {
    display: flex;
    align-items: center;
    gap: 6px;
    
    transform: translateY(-1px);
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
}

.label {
    font-size: 10px;
    
    color: currentColor;
    opacity: 0.5;
    font-weight: 800;
    padding: 2px 5px;
    border-radius: 4px;
    transition: background-color 0.18s ease, color 0.18s ease, opacity 0.18s ease;
    background: rgba(150, 150, 150, 0.15);
    
}


.label.high-traffic {
    color: currentColor;
    opacity: 1;
    background: rgba(255, 255, 255, 0.25);
}

:deep(.island-container[style*="background-color: rgba(255, 255, 255"]) .label.high-traffic {
    background: rgba(0, 0, 0, 0.15);
}

.value {
    font-size: 12px;
    transform: translateY(-0.5px);
    font-weight: 600;
    letter-spacing: 0.2px;
    font-variant-numeric: tabular-nums;
    min-width: 65px;
    text-align: left;
}


.speed-fade-enter-active,
.speed-fade-leave-active {
    transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.2, 0, 0, 1);
}

.speed-fade-enter-from {
    opacity: 0;
    transform: translateY(4px);
    
}

.speed-fade-leave-to {
    opacity: 0;
    transform: translateY(-4px);
    
}

.status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    transition: background-color 0.22s ease;
}


.good {
    background-color: #34C759;
}

.warning {
    background-color: #FFCC00;
}

.error {
    background-color: #FF3B30;
}


.music-ctl-box,
.speed-box {
    position: absolute;
    
    left: 0;
    top: 0;
    display: flex;
    align-items: center;
    width: 100%;
    height: 100%;
}

.music-ctl-box {
    justify-content: flex-start;
}


.inner-wrapper {
    position: relative;
    flex-grow: 1;
    height: 100%;
    display: flex;
    align-items: center;
}

.album-cover {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    box-sizing: unset !important;
    border: 2px solid rgba(255, 255, 255, 0.5) !important;
    background: linear-gradient(135deg, #a8edea 0%, #fed6e3 100%);
    flex-shrink: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.250);
    transition: transform 0.22s cubic-bezier(0.2, 0, 0, 1), border-radius 0.22s ease, box-shadow 0.22s ease;
    z-index: 2;
    transform: translateX(-8px);
}


:deep(.island-container[style*="background-color: rgba(255, 255, 255"]) .album-cover {
    border-color: rgba(0, 0, 0, 0.15);
}

.album-cover.is-playing {
    transform: scale(1.08) translateX(-8px);
}


.cover-inner {
    width: 100%;
    height: 100%;
    background-position: center;
    background-repeat: no-repeat;
    background-size: cover;
    transition: opacity 0.18s ease;
    animation: rotate 8s linear infinite;
    animation-play-state: paused;
    
}


.is-playing .cover-inner {
    animation-play-state: running;
    
}

@keyframes rotate {
    from {
        transform: rotate(0deg);
    }

    to {
        transform: rotate(360deg);
    }
}

.music-controls {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    align-items: center;
    gap: 12px;
    z-index: 10;
}

.ctl-btn {
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6px;
    border-radius: 50%;
    transition: background-color 0.16s ease, opacity 0.16s ease, transform 0.08s ease-out;
    outline: none;
    -webkit-app-region: no-drag;
}


.ctl-btn:hover {
    background-color: rgba(255, 255, 255, 0.15);
}

:deep(.island-container[style*="background-color: rgba(255, 255, 255"]) .ctl-btn:hover {
    background-color: rgba(0, 0, 0, 0.1);
}

.ctl-btn:active {
    opacity: 0.6;
    transform: scale(0.92);
}

.ctl-btn svg {
    width: 16px;
    height: 16px;
    pointer-events: none;
}


.play-btn svg {
    width: 20px;
    height: 20px;
}


.fade-enter-active,
.fade-leave-active {
    transition: opacity 0.25s ease;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}


.music-info-mask-box {
    position: absolute;
    left: 30px;
    right: 18px;
    height: 100%;
    display: flex;
    align-items: center;
    overflow: hidden;
    padding-left: 0;
    -webkit-app-region: no-drag;
    transform: translateY(-1px) translateX(-0.5px);
    mask-image: linear-gradient(to right, #000000 75%, transparent 100%);
    -webkit-mask-image: linear-gradient(to right, #000000 75%, transparent 100%);
}


.music-info-text {
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
    font-size: 12.5px;
    font-weight: 500;
    white-space: nowrap;
    
    overflow: hidden;
    color: inherit;
    opacity: 0.9;
}


.msg-box {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    padding: 0 45px 0 0px;
    box-sizing: border-box;
    z-index: 10;
    gap: 12px;
    -webkit-app-region: no-drag;
}


.msg-avatar {
    width: 35px;
    height: 35px;
    border-radius: 50%;
    background: none;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
    flex-shrink: 0;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

.msg-avatar-img {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    object-fit: cover;
}


.msg-text-wrapper {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: flex-start;
    overflow: hidden;
    flex-grow: 1;
}


.msg-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
    font-weight: 700;
    line-height: 1.4;
    width: 100%;
    overflow: hidden;
}


.sender-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}


.app-name {
    font-size: 10.5px;
    font-weight: 600;
    flex-shrink: 0;
    padding: 2px 6px;
    border-radius: 6px;
    background-color: rgba(150, 150, 150, 0.25);
    color: inherit;
    opacity: 0.9;
    letter-spacing: 0.2px;
    transform: translateY(-0.5px);
}


.msg-body {
    font-size: 12.5px;
    line-height: 1.4;
    opacity: 0.75;
    text-align: left;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.value.high-usage {
    color: #f06861 !important;
}



.audio-spectrum {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    height: 12px;
    padding-right: 2px;
}


.audio-spectrum .bar {
    width: 2px;
    height: 18px;
    background-color: #b6e0ee;
    border-radius: 3px;
    transform-origin: center;
    
    transition: transform 0.08s ease-out;
    will-change: transform;
}

.music-ctl-box {
    transition: opacity 0.16s ease !important;
}

.music-ctl-box.expanded {
    flex-direction: column;
    align-items: flex-start;
    justify-content: flex-start;
    padding: 0 !important;
}


.music-top-row {
    display: flex;
    align-items: center;
    width: 100%;
    height: 100%;
    position: relative;
    transition: none !important;
    
}

.music-ctl-box.expanded .music-top-row {
    height: 40px;
    margin-top: 14px !important;
    margin-left: 5px !important;
    border: none;
}


.album-cover {
    transition: transform 0.22s cubic-bezier(0.2, 0, 0, 1), border-radius 0.22s ease !important;
}

.music-ctl-box.expanded .album-cover {
    width: 40px !important;
    height: 40px !important;
    border-radius: 6px !important;
    animation: none !important;
    border: none;
    transform: translateX(0px) rotate(0deg) !important;
}

.music-ctl-box.expanded .album-cover .cover-inner {
    animation: none !important;
    transform: rotate(0deg) !important;
    border: none;
}

.music-ctl-box.expanded .album-cover.is-playing {
    border: none;
    transform: scale(1.05) translateX(0px) rotate(0deg) !important;
}


.music-ctl-box.expanded .music-info-mask-box {
    left: 60px !important;
    right: 55px !important;
    display: flex !important;
    align-items: center !important;
    justify-content: flex-start !important;
    transition: none !important;
}


.music-info-text {
    position: absolute;
    left: 0 !important;
    top: 50%;
    width: 100%;
    transform: translateY(-50%);
    transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.2, 0, 0, 1);
    text-align: left !important;
    display: flex !important;
    flex-direction: column !important;
    align-items: flex-start !important;
}

.double-line {
    opacity: 0;
    pointer-events: none;
    transform: translateY(-30%);
}

.single-line {
    opacity: 1;
    align-items: center;
    text-align: center;
}

.single-line.fade-out {
    opacity: 0;
    pointer-events: none;
    transform: translateY(20%);
}

.double-line.fade-in {
    opacity: 1;
    pointer-events: auto;
    transform: translateY(-50%) !important;
}

.song-title {
    font-size: 15px;
    font-weight: 700;
    margin-bottom: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
    width: 100%;
    text-align: left !important;
}

.song-artist {
    font-size: 12.5px;
    opacity: 0.65;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.2;
    width: 100%;
    text-align: left !important;
}


.music-ctl-box.expanded .music-controls {
    position: absolute;
    left: 50%;
    transform: translateX(-50%) translateY(5px);
    width: 100%;
    display: flex;
    justify-content: center;
    gap: 20px;
}

.music-ctl-box.expanded .ctl-btn svg {
    width: 22px;
    height: 22px;
}

.music-ctl-box.expanded .play-btn svg {
    width: 28px;
    height: 28px;
}

.audio-spectrum.expanded {
    position: absolute;
    right: 18px !important;
    top: 27px !important;
    transform: scale(1.3);
    
    transition: opacity 0.22s ease, transform 0.22s cubic-bezier(0.2, 0, 0, 1) !important;
}


.music-info-text.single-line {
    overflow: visible !important;
    align-items: flex-start !important;
    text-align: left !important;
}


.scroll-inner {
    display: inline-block;
    white-space: nowrap;
    width: max-content;
    flex-shrink: 0;
    backface-visibility: hidden;
    transform: translateZ(0);
    -webkit-font-smoothing: antialiased;
    transform-style: preserve-3d;
}


.scroll-inner.is-scrolling {
    animation: scroll-ping-pong var(--scroll-duration) linear infinite alternate;
}


@keyframes scroll-ping-pong {

    0%,
    20% {
        transform: translateX(0);
    }

    80%,
    100% {
        
        transform: translateX(calc(-1 * var(--scroll-dist)));
    }
}


.system-toast-box {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    padding-left: 0;
    z-index: 10;
    -webkit-app-region: no-drag;
}

.toast-icon {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transform: translateX(-8px);
}


.toast-icon.app-icon {
    color: currentColor;
}


.toast-icon.sys-icon {
    color: currentColor;
    opacity: 0.85;
}

.toast-icon svg {
    width: 22px;
    height: 22px;
    display: block;
}

.toast-icon.battery-charge-icon {
    color: #34C759;
}

.toast-icon.battery-low-icon {
    color: #FF3B30;
}

.toast-text {
    font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
    opacity: 0.95;
    transform: translateX(-2px) translateY(-1px);
}


.lyric-render-text {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    
    white-space: nowrap;
    overflow: hidden;
    text-align: left !important;
    display: inline-block;
    will-change: opacity, filter;
}

.lyric-fade-enter-active,
.lyric-fade-leave-active {
    
    transition: opacity 0.2s ease, filter 0.22s ease;
}

@media (prefers-reduced-motion: reduce) {
    .island-container,
    .island-core-content,
    .music-ctl-box,
    .music-info-text,
    .album-cover,
    .cover-inner,
    .ctl-btn,
    .audio-spectrum,
    .audio-spectrum .bar,
    .island-hover-panel,
    .panel-tab,
    .panel-icon-btn,
    .panel-pill-btn,
    .song-row,
    .segmented button,
    .panel-material-enter-active,
    .panel-material-leave-active,
    .speed-fade-enter-active,
    .speed-fade-leave-active,
    .lyric-fade-enter-active,
    .lyric-fade-leave-active {
        animation: none !important;
        transition: opacity 0.16s ease, background-color 0.16s ease !important;
        transform: none !important;
        filter: none !important;
    }
}


.lyric-fade-enter-from {
    opacity: 0;
    filter: blur(8px);
}

.lyric-fade-enter-to {
    opacity: 1;
    filter: blur(0px);
}


.lyric-fade-leave-from {
    opacity: 1;
    filter: blur(0px);
}

.lyric-fade-leave-to {
    opacity: 0;
    filter: blur(8px);
}
</style>
