package server

import (
	"bufio"
	"errors"
	"net"
	"net/http"
	"runtime/debug"
	"strconv"
	"strings"
	"time"

	"github.com/thegeorgejoseph/mico/mico-desktop/backend/src/domain"
)

type statusRecorder struct {
	http.ResponseWriter
	status int
}

func (r *statusRecorder) WriteHeader(status int) {
	r.status = status
	r.ResponseWriter.WriteHeader(status)
}

func (r *statusRecorder) Hijack() (net.Conn, *bufio.ReadWriter, error) {
	hijacker, ok := r.ResponseWriter.(http.Hijacker)
	if !ok {
		return nil, nil, errors.New("response writer does not support hijacking")
	}
	return hijacker.Hijack()
}

func (r *statusRecorder) Flush() {
	if flusher, ok := r.ResponseWriter.(http.Flusher); ok {
		flusher.Flush()
	}
}

func (r *statusRecorder) Unwrap() http.ResponseWriter {
	return r.ResponseWriter
}

func (a *App) withRequestLogging(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		recorder := &statusRecorder{ResponseWriter: w, status: http.StatusOK}
		next.ServeHTTP(recorder, r)
		if shouldSkipRequestLog(r) {
			return
		}
		level := domain.LogDebug
		if recorder.status >= 500 {
			level = domain.LogError
		} else if recorder.status >= 400 {
			level = domain.LogWarn
		}
		a.logs.Record(level, "http", r.Method+" "+r.URL.Path, "HTTP request completed.", map[string]string{
			"status":     strconv.Itoa(recorder.status),
			"durationMs": strconv.FormatInt(time.Since(start).Milliseconds(), 10),
		})
	})
}

func (a *App) withRecover(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		defer func() {
			if recovered := recover(); recovered != nil {
				a.logs.Record(domain.LogError, "http", "panic", "Recovered from handler panic.", map[string]string{
					"method": r.Method,
					"path":   r.URL.Path,
					"panic":  stringifyPanic(recovered),
					"stack":  string(debug.Stack()),
				})
				writeErrorStatus(w, http.StatusInternalServerError, errors.New("internal server error"))
			}
		}()
		next.ServeHTTP(w, r)
	})
}

func stringifyPanic(recovered any) string {
	switch value := recovered.(type) {
	case error:
		return value.Error()
	case string:
		return value
	default:
		return "panic"
	}
}

func shouldSkipRequestLog(r *http.Request) bool {
	return r.Method == http.MethodGet && (r.URL.Path == "/api/logs" || r.URL.Path == "/api/health")
}

func withCORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func excerptForLog(raw string) string {
	trimmed := strings.Join(strings.Fields(strings.TrimSpace(raw)), " ")
	if trimmed == "" {
		return ""
	}
	if len(trimmed) > 120 {
		return trimmed[:120] + "..."
	}
	return trimmed
}
