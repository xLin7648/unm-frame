package org.bevyengine.example;

import android.content.Context;
import android.os.Build;
import android.view.Display;
import android.view.View;
import android.view.WindowManager;

import com.google.androidgamesdk.GameActivity;

public class MainActivity extends GameActivity {
    static {
        System.loadLibrary("unm");
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);

        if (hasFocus) {
            hideSystemUi();
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
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // API 30+ 推荐方式
            Display display = this.getDisplay(); // `this` 指的是当前的 Context 实例（例如 Activity）
            if (display != null) {
                return display.getRefreshRate();
            } else {
                return 60.0f; // 默认值，与 Kotlin 的 ?: 对应
            }
        } else {
            // API 30- 兼容方式
            WindowManager windowManager = (WindowManager) getSystemService(Context.WINDOW_SERVICE);
            // 对于旧版 API (<R), WindowManager.getDefaultDisplay() 是获取 Display 的方式
            return windowManager.getDefaultDisplay().getRefreshRate();
        }
    }
}