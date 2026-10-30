// Windows SDK supplies the real DirectDraw/Direct3D7 COM layouts. No game
// object layout or cached Board/App address crosses this implementation.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#define DIRECTDRAW_VERSION 0x0700
#define DIRECT3D_VERSION 0x0700
#include <windows.h>
#include <ddraw.h>
#include <d3d.h>
#include <wrl/client.h>
#include <algorithm>
#include <array>
#include <climits>
#include <cstdint>
#include <cstring>
#include <limits>
#include <memory>
#include <string>
#include <string_view>
#include <unordered_map>
#include <vector>

namespace {
using Microsoft::WRL::ComPtr;
struct Glyph {
    ComPtr<IDirectDrawSurface7> surface;
    int width = 0, height = 0;
    float u = 0, v = 0;
};
struct Font {
    std::uint32_t sources = 0;
    std::wstring family;
    int pixels;
    bool bold;
    HFONT handle = nullptr;
    int digit_top = 0, digit_bottom = 0;
    std::unordered_map<wchar_t, Glyph> glyphs;
    Font(std::wstring_view name, int size, bool weight) : family(name), pixels(size), bold(weight) {}
    ~Font() { if (handle) DeleteObject(handle); }
};
struct Vertex {
    float x, y, z = 0, rhw = 1;
    DWORD color, specular = 0;
    float u = 0, v = 0;
};

struct Canvas {
    std::uint32_t source = 0;
    ComPtr<IDirect3DDevice7> device;
    ComPtr<IDirectDraw7> draw;
    DWORD saved_state = 0;
    bool active = false;
    int width = 0, height = 0;
    RECT clip{};
    std::vector<std::unique_ptr<Font>> fonts;
    std::array<Glyph,2> cursors;
    std::array<HCURSOR,2> cursor_shapes{};
    std::wstring wide_text; // Encoding workspace, not a cached game-state query.
    ~Canvas() { if (saved_state && device) device->DeleteStateBlock(saved_state); }

    HRESULT bind(IDirect3DDevice7* next_device, IDirectDraw7* next_draw) {
        if (!next_device || !next_draw) return E_POINTER;
        if (device.Get() == next_device && draw.Get() == next_draw && saved_state) return S_OK;
        if (active) return E_UNEXPECTED;
        fonts.clear();
        cursors = {};
        if (saved_state && device) device->DeleteStateBlock(saved_state);
        saved_state = 0;
        device = next_device; // Own COM references for the drawing resources.
        draw = next_draw;
        return device->CreateStateBlock(D3DSBT_ALL, &saved_state);
    }
    HRESULT begin(int canvas_width, int canvas_height) {
        if (active || !saved_state || canvas_width <= 0 || canvas_height <= 0) return E_INVALIDARG;
        HRESULT result = device->CaptureStateBlock(saved_state);
        if (FAILED(result)) return result;
        active = true;
        width = canvas_width;
        height = canvas_height;
        clip = {0, 0, width, height};
        const std::pair<D3DRENDERSTATETYPE, DWORD> states[] = {
            {D3DRENDERSTATE_ZENABLE, D3DZB_FALSE}, {D3DRENDERSTATE_ZWRITEENABLE, FALSE},
            {D3DRENDERSTATE_ALPHABLENDENABLE, TRUE}, {D3DRENDERSTATE_SRCBLEND, D3DBLEND_SRCALPHA},
            {D3DRENDERSTATE_DESTBLEND, D3DBLEND_INVSRCALPHA}, {D3DRENDERSTATE_LIGHTING, FALSE},
            {D3DRENDERSTATE_CULLMODE, D3DCULL_NONE},
            {D3DRENDERSTATE_ALPHATESTENABLE, FALSE}, {D3DRENDERSTATE_FOGENABLE, FALSE},
        };
        for (auto [key, value] : states) {
            result = device->SetRenderState(key, value);
            if (FAILED(result)) { end(); return result; }
        }
        for (auto stage : {D3DTSS_MINFILTER, D3DTSS_MAGFILTER}) {
            result = device->SetTextureStageState(0, stage, D3DTFG_POINT);
            if (FAILED(result)) { end(); return result; }
        }
        return S_OK;
    }
    HRESULT end() {
        if (!active) return S_OK;
        active = false;
        return device->ApplyStateBlock(saved_state);
    }
    HRESULT quad(int x, int y, int w, int h, DWORD color, IDirectDrawSurface7* texture = nullptr,
                 float u = 0, float v = 0) {
        if (!active) return E_UNEXPECTED;
        if (w <= 0 || h <= 0 || !(color >> 24)) return S_OK;
        const auto right = static_cast<std::int64_t>(x) + w;
        const auto bottom = static_cast<std::int64_t>(y) + h;
        const int left = std::max(x, static_cast<int>(clip.left));
        const int top = std::max(y, static_cast<int>(clip.top));
        const int clipped_right = static_cast<int>(std::min<std::int64_t>(right, clip.right));
        const int clipped_bottom = static_cast<int>(std::min<std::int64_t>(bottom, clip.bottom));
        if (left >= clipped_right || top >= clipped_bottom) return S_OK;
        const float u0 = static_cast<float>(static_cast<std::int64_t>(left) - x) / w * u;
        const float v0 = static_cast<float>(static_cast<std::int64_t>(top) - y) / h * v;
        const float u1 = static_cast<float>(static_cast<std::int64_t>(clipped_right) - x) / w * u;
        const float v1 = static_cast<float>(static_cast<std::int64_t>(clipped_bottom) - y) / h * v;
        Vertex vertices[] = {
            {static_cast<float>(left), static_cast<float>(top), 0, 1, color, 0, u0, v0},
            {static_cast<float>(left), static_cast<float>(clipped_bottom), 0, 1, color, 0, u0, v1},
            {static_cast<float>(clipped_right), static_cast<float>(top), 0, 1, color, 0, u1, v0},
            {static_cast<float>(clipped_right), static_cast<float>(clipped_bottom), 0, 1, color, 0, u1, v1},
        };
        HRESULT result = device->SetTexture(0, texture);
        if (FAILED(result)) return result;
        const std::pair<D3DTEXTURESTAGESTATETYPE, DWORD> stages[] = {
            {D3DTSS_COLORARG1, D3DTA_TEXTURE}, {D3DTSS_COLORARG2, D3DTA_DIFFUSE},
            {D3DTSS_ALPHAARG1, D3DTA_TEXTURE}, {D3DTSS_ALPHAARG2, D3DTA_DIFFUSE},
            {D3DTSS_COLOROP, texture ? D3DTOP_MODULATE : D3DTOP_SELECTARG2},
            {D3DTSS_ALPHAOP, texture ? D3DTOP_MODULATE : D3DTOP_SELECTARG2},
        };
        for (auto [key, value] : stages) {
            result = device->SetTextureStageState(0, key, value);
            if (FAILED(result)) return result;
        }
        return device->DrawPrimitive(D3DPT_TRIANGLESTRIP,
            D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_SPECULAR | D3DFVF_TEX1, vertices, 4, 0);
    }
    HRESULT font(const wchar_t* family, int pixels, bool bold, Font*& output) {
        if (!family || pixels <= 0) return E_INVALIDARG;
        std::wstring_view name(family);
        if (name.size() >= LF_FACESIZE) return E_INVALIDARG;
        for (auto& candidate : fonts) {
            if (candidate->family == name && candidate->pixels == pixels && candidate->bold == bold) {
                candidate->sources |= 1u << source; output = candidate.get(); return S_OK;
            }
        }
        auto created = std::make_unique<Font>(name, pixels, bold);
        created->sources = 1u << source;
        // AvZ's nonbold painter uses weight 100 and antialiased TrueType glyphs.
        created->handle = CreateFontW(-pixels, 0, 0, 0, bold ? FW_BOLD : 100,
            FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_TT_PRECIS, CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY, FIXED_PITCH | FF_DONTCARE, family);
        if (!created->handle) return HRESULT_FROM_WIN32(GetLastError());
        output = created.get(); fonts.push_back(std::move(created));
        return S_OK;
    }
    HRESULT digit_bounds(const wchar_t* family, int pixels, bool bold, int* top, int* bottom) {
        if (!top || !bottom) return E_INVALIDARG;
        Font* face = nullptr;
        HRESULT result = font(family, pixels, bold, face);
        if (FAILED(result)) return result;
        if (!face->digit_bottom) {
            HDC dc = CreateCompatibleDC(nullptr);
            if (!dc) return E_FAIL;
            HGDIOBJ old = SelectObject(dc, face->handle);
            struct Guard { HDC dc; HGDIOBJ old; ~Guard() { SelectObject(dc, old); DeleteDC(dc); } } guard{dc, old};
            TEXTMETRICW metrics{};
            if (!GetTextMetricsW(dc, &metrics)) return E_FAIL;
            MAT2 identity{{0,1},{0,0},{0,0},{0,1}};
            int first = INT_MAX, last = INT_MIN;
            for (wchar_t ch = L'0'; ch <= L'9'; ++ch) {
                GLYPHMETRICS glyph{};
                if (GetGlyphOutlineW(dc, ch, GGO_METRICS, &glyph, 0, nullptr, &identity) == GDI_ERROR) return E_FAIL;
                const int y = 1 + metrics.tmAscent - glyph.gmptGlyphOrigin.y;
                first = std::min(first, y);
                last = std::max(last, y + static_cast<int>(glyph.gmBlackBoxY));
            }
            face->digit_top = first; face->digit_bottom = last;
        }
        *top = face->digit_top; *bottom = face->digit_bottom;
        return S_OK;
    }
    HRESULT glyph(Font& font, wchar_t character, Glyph*& output) {
        auto found = font.glyphs.find(character);
        if (found != font.glyphs.end()) { output = &found->second; return S_OK; }
        HDC dc = CreateCompatibleDC(nullptr);
        if (!dc) return E_FAIL;
        struct DcGuard { HDC dc; ~DcGuard() { DeleteDC(dc); } } dc_guard{dc};
        HGDIOBJ old_font = SelectObject(dc, font.handle);
        struct SelectionGuard { HDC dc; HGDIOBJ object; ~SelectionGuard() { SelectObject(dc, object); } } font_guard{dc, old_font};
        RECT measured{0, 0, font.pixels * 11 / 10, font.pixels * 11 / 10 + 1};
        DrawTextW(dc, &character, 1, &measured, DT_CALCRECT | DT_NOPREFIX);
        Glyph created;
        created.width = measured.right;
        created.height = font.pixels * 11 / 10 + 1;
        if (created.width <= 0) { output = &font.glyphs.emplace(character, std::move(created)).first->second; return S_OK; }
        D3DDEVICEDESC7 caps{};
        HRESULT result = device->GetCaps(&caps);
        if (FAILED(result)) return result;
        if (static_cast<DWORD>(created.width) > caps.dwMaxTextureWidth || static_cast<DWORD>(created.height) > caps.dwMaxTextureHeight)
            return DDERR_UNSUPPORTED;
        const std::uint64_t bytes = static_cast<std::uint64_t>(created.width) * created.height * 4;
        if (bytes > std::numeric_limits<std::size_t>::max()) return E_OUTOFMEMORY;
        BITMAPINFO info{};
        info.bmiHeader = {sizeof(BITMAPINFOHEADER), created.width, -created.height, 1, 32, BI_RGB, 0, 0, 0, 0, 0};
        void* pixels = nullptr;
        HBITMAP bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
        if (!bitmap || !pixels) { if (bitmap) DeleteObject(bitmap); return E_OUTOFMEMORY; }
        struct BitmapGuard { HBITMAP bitmap; ~BitmapGuard() { DeleteObject(bitmap); } } bitmap_guard{bitmap};
        HGDIOBJ old_bitmap = SelectObject(dc, bitmap);
        SelectionGuard bitmap_selection{dc, old_bitmap};
        std::memset(pixels, 0, static_cast<std::size_t>(bytes));
        SetBkMode(dc, TRANSPARENT); SetTextColor(dc, RGB(255, 255, 255));
        RECT area{0, 1, created.width, created.height};
        DrawTextW(dc, &character, 1, &area, DT_WORDBREAK | DT_EDITCONTROL | DT_NOPREFIX);
        GdiFlush();
        result = upload(created,static_cast<const DWORD*>(pixels),true);
        if (FAILED(result)) return result;
        output = &font.glyphs.emplace(character, std::move(created)).first->second;
        return S_OK;
    }
    HRESULT upload(Glyph& created,const DWORD* pixels,bool mask) {
        DDSURFACEDESC2 description{};
        description.dwSize = sizeof(description);
        description.dwFlags = DDSD_CAPS | DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT;
        description.ddsCaps.dwCaps = DDSCAPS_TEXTURE;
        description.ddsCaps.dwCaps2 = DDSCAPS2_D3DTEXTUREMANAGE;
        description.dwWidth = created.width; description.dwHeight = created.height;
        description.ddpfPixelFormat.dwSize = sizeof(DDPIXELFORMAT);
        description.ddpfPixelFormat.dwFlags = DDPF_ALPHAPIXELS | DDPF_RGB;
        description.ddpfPixelFormat.dwRGBBitCount = 32;
        description.ddpfPixelFormat.dwRGBAlphaBitMask = 0xff000000;
        description.ddpfPixelFormat.dwRBitMask = 0x00ff0000;
        description.ddpfPixelFormat.dwGBitMask = 0x0000ff00;
        description.ddpfPixelFormat.dwBBitMask = 0x000000ff;
        ComPtr<IDirectDrawSurface7> surface;
        HRESULT result = draw->CreateSurface(&description, surface.GetAddressOf(), nullptr);
        if (FAILED(result)) return result;
        DDSURFACEDESC2 locked{}; locked.dwSize = sizeof(locked);
        result = surface->Lock(nullptr, &locked, DDLOCK_WAIT | DDLOCK_WRITEONLY, nullptr);
        if (FAILED(result)) return result;
        if (!locked.lpSurface || locked.dwWidth < static_cast<DWORD>(created.width)
            || locked.dwHeight < static_cast<DWORD>(created.height) || locked.lPitch < created.width * 4) {
            surface->Unlock(nullptr); return E_FAIL;
        }
        for (int y = 0; y < created.height; ++y) {
            auto* destination = reinterpret_cast<DWORD*>(static_cast<char*>(locked.lpSurface) + y * locked.lPitch);
            auto* source = static_cast<const DWORD*>(pixels) + y * created.width;
            for (int x = 0; x < created.width; ++x) destination[x] = mask ? ((source[x] << 24) | 0x00ffffff) : source[x];
        }
        created.u = static_cast<float>(created.width - 1) / locked.dwWidth;
        created.v = static_cast<float>(created.height - 1) / locked.dwHeight;
        result = surface->Unlock(nullptr);
        if (FAILED(result)) return result;
        created.surface=std::move(surface);
        return S_OK;
    }
    HRESULT cursor(int x,int y,bool hand) {
        if (!active) return E_UNEXPECTED;
        auto& glyph=cursors[hand];
        if (!glyph.surface) {
            glyph.width=GetSystemMetrics(SM_CXCURSOR)+1;
            glyph.height=GetSystemMetrics(SM_CYCURSOR)+1;
            if (glyph.width<=0 || glyph.height<=0) return E_FAIL;
            HDC dc=CreateCompatibleDC(nullptr);
            if (!dc) return E_FAIL;
            struct DC {HDC value;~DC(){DeleteDC(value);}} dc_guard{dc};
            BITMAPINFO info{};
            info.bmiHeader={sizeof(BITMAPINFOHEADER),glyph.width,-glyph.height,1,32,BI_RGB,0,0,0,0,0};
            void* pixels=nullptr;
            HBITMAP bitmap=CreateDIBSection(dc,&info,DIB_RGB_COLORS,&pixels,nullptr,0);
            if (!bitmap || !pixels){if(bitmap)DeleteObject(bitmap);return E_OUTOFMEMORY;}
            HGDIOBJ previous=SelectObject(dc,bitmap);
            struct Bitmap {HDC dc;HBITMAP bitmap;HGDIOBJ previous;~Bitmap(){SelectObject(dc,previous);DeleteObject(bitmap);}} bitmap_guard{dc,bitmap,previous};
            const auto count=static_cast<std::size_t>(glyph.width)*glyph.height;
            std::memset(pixels,0,count*4);
            HCURSOR standard=LoadCursorW(nullptr,hand?MAKEINTRESOURCEW(32649):MAKEINTRESOURCEW(32512));
            HCURSOR shape=cursor_shapes[hand]?cursor_shapes[hand]:standard;
            if (!DrawIconEx(dc,0,0,shape,0,0,0,nullptr,DI_NORMAL|DI_COMPAT)) return E_FAIL;
            GdiFlush();
            auto* bits=static_cast<DWORD*>(pixels);
            if (shape!=standard) {
                if(hand) {
                    // A-TAS/AvZ's native hand cursor needs an explicit black rim.
                    for(int row=0;row<glyph.height;++row){
                        DWORD* previous=bits;
                        for(int col=0;col<glyph.width;++col){auto* current=bits+row*glyph.width+col;
                            if(*previous!=0xffffff&&*current==0xffffff)*previous=0xff000000;
                            else if(*previous==0xffffff&&*current!=0xffffff)*current=0xff000000;
                            previous=current;
                        }
                    }
                    for(int col=0;col<glyph.width;++col){
                        DWORD* previous=bits;
                        for(int row=0;row<glyph.height;++row){auto* current=bits+row*glyph.width+col;
                            if(*previous!=0xffffff&&*current==0xffffff)*previous=0xff000000;
                            else if(*previous==0xffffff&&*current!=0xffffff)*current=0xff000000;
                            previous=current;
                        }
                    }
                }
                for(std::size_t i=0;i<count;++i)bits[i]=bits[i]==0xffffff?0xffffffff:bits[i]==0?0:0xff000000;
            }
            HRESULT result=upload(glyph,bits,false);if(FAILED(result))return result;
        }
        return quad(x,y,glyph.width,glyph.height,0xffffffff,glyph.surface.Get(),glyph.u,glyph.v);
    }
    HRESULT text(int x, int y, const char* utf8, int length, const wchar_t* family, int pixels,
                 bool bold, DWORD color, bool measure, std::uint32_t* out_width, std::uint32_t* out_height) {
        if (!active || !utf8 || length < 0) return E_INVALIDARG;
        Font* selected = nullptr;
        HRESULT result = font(family, pixels, bold, selected);
        if (FAILED(result)) return result;
        const int count = length ? MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8, length, nullptr, 0) : 0;
        if (length && count <= 0) return E_INVALIDARG;
        wide_text.resize(count);
        if (count && MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8, length, wide_text.data(), count) != count) return E_INVALIDARG;
        std::int64_t dx = 0, dy = 0, widest = 0;
        for (wchar_t character : wide_text) {
            if (character == L'\n') { widest = std::max(widest, dx); dx = 0; dy += pixels; continue; }
            if (character == L'\r') continue;
            if (!measure) {
                Glyph* current = nullptr;
                result = glyph(*selected, character, current);
                if (FAILED(result)) return result;
                const auto px = x + dx, py = y + dy;
                if (px >= INT_MIN && px <= INT_MAX && py >= INT_MIN && py <= INT_MAX) {
                    result = quad(static_cast<int>(px), static_cast<int>(py), current->width, current->height,
                        color, current->surface.Get(), current->u, current->v);
                    if (FAILED(result)) return result;
                }
            }
            // Match APainter's fixed advance, including its ASCII half-cell.
            dx += character > 0xff ? pixels : pixels / 2 + 1;
        }
        widest = std::max(widest, dx);
        if (widest > UINT_MAX || dy + pixels > UINT_MAX) return E_INVALIDARG;
        if (out_width) *out_width = static_cast<std::uint32_t>(widest);
        if (out_height) *out_height = count ? static_cast<std::uint32_t>(dy + (wide_text.back()==L'\n' ? 0 : pixels)) : 0;
        wide_text.clear();
        return S_OK;
    }
};

// Explicitly destroyed by native cleanup before DLL unload, never by a C++
// global/TLS destructor running under the Windows loader lock.
Canvas* canvas = nullptr;
template<class F> HRESULT guarded(F&& operation) noexcept {
    try { return operation(); } catch (const std::bad_alloc&) { return E_OUTOFMEMORY; } catch (...) { return E_FAIL; }
}
}

extern "C" std::int32_t rsvz_canvas_begin(void* device, void* draw, int width, int height) noexcept {
    return guarded([&] {
        if (!canvas) canvas = new Canvas;
        HRESULT result = canvas->bind(static_cast<IDirect3DDevice7*>(device), static_cast<IDirectDraw7*>(draw));
        return FAILED(result) ? result : canvas->begin(width, height);
    });
}
extern "C" std::int32_t rsvz_canvas_end() noexcept { return canvas ? canvas->end() : E_UNEXPECTED; }
extern "C" std::int32_t rsvz_canvas_clear() noexcept {
    if (canvas && canvas->active) return E_UNEXPECTED;
    delete canvas; canvas = nullptr; return S_OK;
}
extern "C" std::int32_t rsvz_canvas_rect(int x, int y, int w, int h, std::uint32_t argb) noexcept {
    return canvas ? canvas->quad(x, y, w, h, argb) : E_UNEXPECTED;
}
extern "C" std::int32_t rsvz_canvas_clip(int enabled, int x, int y, int w, int h) noexcept {
    if (!canvas || !canvas->active) return E_UNEXPECTED;
    if (!enabled) { canvas->clip = {0, 0, canvas->width, canvas->height}; return S_OK; }
    auto clamp = [](std::int64_t value, int limit) { return static_cast<LONG>(std::clamp<std::int64_t>(value, 0, limit)); };
    canvas->clip = {clamp(x, canvas->width), clamp(y, canvas->height),
        clamp(static_cast<std::int64_t>(x) + std::max(0, w), canvas->width),
        clamp(static_cast<std::int64_t>(y) + std::max(0, h), canvas->height)};
    return S_OK;
}
extern "C" std::int32_t rsvz_canvas_text(int x, int y, const char* utf8, int length,
    const wchar_t* family, int pixels, int bold, std::uint32_t argb, int measure,
    std::uint32_t* width, std::uint32_t* height) noexcept {
    return guarded([&] { return canvas ? canvas->text(x, y, utf8, length, family, pixels, bold != 0,
        argb, measure != 0, width, height) : E_UNEXPECTED; });
}

extern "C" std::int32_t rsvz_canvas_cursor(int x,int y,int hand) noexcept {
    return guarded([&]{return canvas?canvas->cursor(x,y,hand!=0):E_UNEXPECTED;});
}
extern "C" std::int32_t rsvz_canvas_digit_bounds(const wchar_t* family, int pixels, int bold, int* top, int* bottom) noexcept {
    return guarded([&] { return canvas ? canvas->digit_bounds(family, pixels, bold != 0, top, bottom) : E_UNEXPECTED; });
}
extern "C" std::int32_t rsvz_canvas_needs_cursor() noexcept {
    return canvas&&(!canvas->cursor_shapes[0]||!canvas->cursor_shapes[1]);
}
extern "C" void rsvz_canvas_capture_cursor(int kind,void* handle) noexcept {
    if(canvas && kind>=0 && kind<2 && handle && !canvas->cursor_shapes[kind]) {
        canvas->cursor_shapes[kind]=static_cast<HCURSOR>(handle);
        canvas->cursors[kind]={};
    }
}

extern "C" void rsvz_canvas_source(std::uint32_t source) noexcept { if (canvas && source < 2) canvas->source = source; }
extern "C" void rsvz_canvas_clear_source(std::uint32_t source) noexcept {
    if (!canvas || canvas->active || source >= 2) return;
    for (auto& font : canvas->fonts) font->sources &= ~(1u << source);
    canvas->fonts.erase(std::remove_if(canvas->fonts.begin(), canvas->fonts.end(),
        [](const auto& font) { return !font->sources; }), canvas->fonts.end());
}
