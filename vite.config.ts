import { defineConfig } from "vite";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
    // Vite options tailored for Tauri development
    clearScreen: false,
    server: {
        port: 5173,
        strictPort: true,
        watch: {
            // 3. tell vite to ignore watching `src-tauri`
            ignored: ["**/src-tauri/**"],
        },
    },
}));
