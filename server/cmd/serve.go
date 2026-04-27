// SPDX-License-Identifier: Apache-2.0

package cmd

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"time"

	"github.com/2233admin/codenexus/internal/health"
	"github.com/2233admin/codenexus/internal/mcpsrv"
	"github.com/2233admin/codenexus/internal/proxy"
	"github.com/2233admin/codenexus/internal/supervisor"
	"github.com/2233admin/codenexus/internal/ui"
	"github.com/go-chi/chi/v5"
	"github.com/spf13/cobra"
)

// serveCmd is the long-running subcommand: HTTP + MCP + Rust supervisor.
var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Run HTTP+MCP service and supervise Rust core",
	RunE:  runServe,
}

// runServe wires every moving part: logger, port scan, supervisor, chi router,
// optional MCP-stdio goroutine, and signal-driven graceful shutdown.
func runServe(cmd *cobra.Command, args []string) error {
	logger := newLogger(LogLevel())
	slog.SetDefault(logger)

	rootCtx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	rustPort, lockfilePath, err := supervisor.AcquireRustPort()
	if err != nil {
		return fmt.Errorf("serve: acquire rust port: %w", err)
	}

	rustBin := resolveRustBinPath(RustBin())
	if rustBin == "" {
		slog.Debug("serve: no explicit rust binary path; supervisor will extract from embed",
			"hint", "set --rust-bin or CODENEXUS_RUST_BIN for dev override")
	}

	cfg := supervisor.Config{
		RustBinPath:  rustBin,
		RustPort:     rustPort,
		LockfilePath: lockfilePath,
		DataDir:      defaultDataDir(),
		Device:       "auto",
		RustLog:      "info",
	}

	sup, err := supervisor.Start(rootCtx, cfg)
	if err != nil {
		return fmt.Errorf("serve: start supervisor: %w", err)
	}
	defer func() {
		if stopErr := sup.Stop(); stopErr != nil {
			slog.Warn("serve: supervisor stop returned error", "err", stopErr)
		}
	}()

	client := proxy.New(rustPort)

	r := chi.NewRouter()
	r.Mount("/healthz", health.NewHandler(sup, rustPort))
	r.Mount("/mcp", mcpsrv.NewHTTPHandler())
	r.Mount("/ui/", http.StripPrefix("/ui/", http.FileServer(http.FS(ui.UIFS))))
	r.Get("/", func(w http.ResponseWriter, req *http.Request) {
		http.Redirect(w, req, "/ui/", http.StatusFound)
	})
	r.Post("/api/v1/query", apiPassthrough(client, "query"))
	r.Post("/api/v1/index", apiPassthrough(client, "index_repo"))
	r.Post("/api/v1/list_callers", apiPassthrough(client, "list_callers"))

	if os.Getenv("CODENEXUS_MCP_STDIO") == "1" {
		go func() {
			if err := mcpsrv.RunStdio(rootCtx, client); err != nil {
				slog.Error("serve: mcp stdio exited", "err", err)
			}
		}()
	}

	addr := fmt.Sprintf(":%d", Port())
	srv := &http.Server{
		Addr:              addr,
		Handler:           r,
		ReadHeaderTimeout: 10 * time.Second,
	}

	errCh := make(chan error, 1)
	go func() {
		slog.Info("serve: http listening", "addr", addr, "rust_port", rustPort)
		if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			errCh <- err
		}
		close(errCh)
	}()

	select {
	case <-rootCtx.Done():
		slog.Info("serve: shutdown signal received")
	case err := <-errCh:
		if err != nil {
			return fmt.Errorf("serve: http listener: %w", err)
		}
	}

	shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if err := srv.Shutdown(shutdownCtx); err != nil {
		return fmt.Errorf("serve: http shutdown: %w", err)
	}
	return nil
}

// newLogger builds a JSON slog logger writing to stdout at the requested level.
// Required fields per ARCH §6: time, level, source, msg (trace_id added by callers).
func newLogger(level string) *slog.Logger {
	var lvl slog.Level
	switch strings.ToLower(level) {
	case "debug":
		lvl = slog.LevelDebug
	case "warn", "warning":
		lvl = slog.LevelWarn
	case "error":
		lvl = slog.LevelError
	default:
		lvl = slog.LevelInfo
	}
	h := slog.NewJSONHandler(os.Stdout, &slog.HandlerOptions{
		Level:     lvl,
		AddSource: true,
	})
	return slog.New(h)
}

// resolveRustBinPath returns the first non-empty source: explicit flag, env, or
// auto-discover relative to the running executable. Returns "" if none found.
func resolveRustBinPath(flagVal string) string {
	if flagVal != "" {
		return flagVal
	}
	if env := os.Getenv("CODENEXUS_RUST_BIN"); env != "" {
		return env
	}

	exe, err := os.Executable()
	if err != nil {
		return ""
	}
	exeDir := filepath.Dir(exe)
	candidates := []string{
		filepath.Join(exeDir, "..", "core", "target", "release", "codenexus-core"),
		filepath.Join(exeDir, "..", "core", "target", "release", "codenexus-core.exe"),
	}
	if runtime.GOOS == "windows" {
		// Reorder so .exe wins on Windows.
		candidates = []string{candidates[1], candidates[0]}
	}
	for _, c := range candidates {
		if fi, err := os.Stat(c); err == nil && !fi.IsDir() {
			abs, err := filepath.Abs(c)
			if err == nil {
				return abs
			}
			return c
		}
	}
	return ""
}

// defaultDataDir returns XDG_DATA_HOME/codenexus or its OS-default fallback.
func defaultDataDir() string {
	if x := os.Getenv("XDG_DATA_HOME"); x != "" {
		return filepath.Join(x, "codenexus")
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(os.TempDir(), "codenexus")
	}
	switch runtime.GOOS {
	case "windows":
		if local := os.Getenv("LOCALAPPDATA"); local != "" {
			return filepath.Join(local, "codenexus")
		}
		return filepath.Join(home, "AppData", "Local", "codenexus")
	case "darwin":
		return filepath.Join(home, "Library", "Application Support", "codenexus")
	default:
		return filepath.Join(home, ".local", "share", "codenexus")
	}
}

// apiPassthrough wires POST /api/v1/{op} to proxy.SendTask. Body is treated as
// the operation-specific args object; the handler injects "operation": op.
func apiPassthrough(client *proxy.Client, op string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var args map[string]any
		if r.ContentLength > 0 {
			if err := json.NewDecoder(r.Body).Decode(&args); err != nil {
				http.Error(w, fmt.Sprintf("api: decode body: %v", err), http.StatusBadRequest)
				return
			}
		}
		raw, err := client.SendTask(r.Context(), op, args)
		if err != nil {
			http.Error(w, fmt.Sprintf("api: %s: %v", op, err), http.StatusBadGateway)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write(raw)
	}
}
