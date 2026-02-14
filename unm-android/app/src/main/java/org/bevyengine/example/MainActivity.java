package org.bevyengine.example;

import android.animation.Animator;
import android.animation.AnimatorListenerAdapter;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.Display;
import android.view.PixelCopy;
import android.view.SurfaceView;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.widget.FrameLayout;
import android.widget.ImageView;

import com.google.androidgamesdk.GameActivity;

public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("unm");
    }

    private ImageView transitionOverlay; // 用于覆盖画面的 View
    private Bitmap lastFrameBitmap;      // 存储挂起前的快照

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // 在 super 之前设置窗口背景（可选，双重保险）
        getWindow().setBackgroundDrawableResource(android.R.color.black);

        super.onCreate(savedInstanceState);

        transitionOverlay = new ImageView(this);
        transitionOverlay.setScaleType(ImageView.ScaleType.FIT_XY);

        // 冷启动：先彻底涂黑，确保没有任何透明度变化
        transitionOverlay.setBackgroundColor(Color.BLACK);
        transitionOverlay.setAlpha(1.0f);
        transitionOverlay.setVisibility(View.VISIBLE);

        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
        );
        addContentView(transitionOverlay, params);
    }

    @Override
    protected void onPause() {
        // 2. 核心步骤：在应用挂起前，截取当前 SurfaceView 的内容
        // 注意：GameActivity 内部通常包含一个 SurfaceView
        // 我们通过查找 content 视图来找到它
        View view = getWindow().getDecorView().findViewById(android.R.id.content);
        if (view instanceof ViewGroup) {
            findAndCaptureSurface((ViewGroup) view);
        }
        super.onPause();
    }

    /**
     * 递归寻找 SurfaceView 并拍照
     */
    private void findAndCaptureSurface(ViewGroup group) {
        for (int i = 0; i < group.getChildCount(); i++) {
            View child = group.getChildAt(i);
            if (child instanceof SurfaceView) {
                SurfaceView sv = (SurfaceView) child;
                if (sv.getHolder().getSurface().isValid()) {
                    captureSnapshot(sv);
                }
            } else if (child instanceof ViewGroup) {
                findAndCaptureSurface((ViewGroup) child);
            }
        }
    }

    private void captureSnapshot(SurfaceView surfaceView) {
        // 创建一个和 Surface 同样大小的 Bitmap
        final Bitmap bitmap = Bitmap.createBitmap(surfaceView.getWidth(), surfaceView.getHeight(), Bitmap.Config.ARGB_8888);

        // 使用 PixelCopy 异步拷贝 Surface 内容（不阻塞渲染）
        PixelCopy.request(surfaceView, bitmap, (copyResult) -> {
            if (copyResult == PixelCopy.SUCCESS) {
                lastFrameBitmap = bitmap;
                // 将截图设为覆盖层的背景，以便恢复时直接看到这张图
                runOnUiThread(() -> {
                    transitionOverlay.setImageBitmap(lastFrameBitmap);
                    transitionOverlay.setAlpha(1.0f);
                    transitionOverlay.setVisibility(View.VISIBLE);
                });
            }
        }, new Handler(Looper.getMainLooper()));
    }

    /**
     * 由 Rust 端在重新初始化 wgpu 完毕，且 present() 第一帧后调用
     */
    public void GameReady() {
        runOnUiThread(() -> {
            if (transitionOverlay.getVisibility() != View.VISIBLE) return;

            // 执行渐隐动画，露出底层的实时渲染画面
            transitionOverlay.animate()
                    .alpha(0.0f)
                    .setDuration(250) // 500ms 渐隐
                    .setListener(new AnimatorListenerAdapter() {
                        @Override
                        public void onAnimationEnd(Animator animation) {
                            transitionOverlay.setVisibility(View.GONE);
                            transitionOverlay.setImageBitmap(null);
                            // 释放 Bitmap 内存
                            if (lastFrameBitmap != null) {
                                lastFrameBitmap.recycle();
                                lastFrameBitmap = null;
                            }
                        }
                    });
        });
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemUi();
            setupDisplayCutoutHandling();
        }
    }

    private void hideSystemUi() {
        View decorView = getWindow().getDecorView();
        decorView.setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                        | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                        | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_FULLSCREEN
        );
    }

    public float getRefreshRate() {
        Display display = this.getDisplay();
        return (display != null) ? display.getRefreshRate() : 60.0f;
    }

    public void setupDisplayCutoutHandling() {
        // 1. 设置窗口延伸至刘海区域
        WindowManager.LayoutParams attributes = getWindow().getAttributes();
        // 关键设置：允许内容延伸到短边刘海区域
        attributes.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        getWindow().setAttributes(attributes);
    }
}