package app.msime.android;

import android.app.Activity;
import android.os.Bundle;
import android.text.InputType;
import android.util.TypedValue;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 只进 debug 包的验证页：路径 C（密码框）在真机上没有可安全到达的输入框，上一轮四条取证都撞墙
 * （见 PIN「方案甲-2 落地」⑦），所以自建一个最小的框来证「密码框不建会话」这条隐私硬门。
 * 判据：普通框要有候选格与「会话建立」日志，textPassword 框两者都不该出现。
 */
public class TestPasswordActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        int pad = dp(16);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(pad, pad, pad, pad);

        EditText plain = field(InputType.TYPE_CLASS_TEXT, "普通框 text");
        EditText password = field(
                InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD,
                "密码框 textPassword");
        root.addView(label("普通框 text"));
        root.addView(plain);
        root.addView(label("密码框 textPassword"));
        root.addView(password);
        setContentView(root);
        // 不主动 requestFocus 软键盘不会自己弹（宿主 KeyboardTryoutActivity.java:155 同一做法）；
        // 键盘没弹时 dump 里本来就没有候选格，"密码框没有候选"会退化成假零。
        plain.requestFocus();
    }

    private TextView label(String caption) {
        TextView view = new TextView(this);
        view.setText(caption);
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, 14);
        return view;
    }

    private EditText field(int inputType, String caption) {
        EditText view = new EditText(this);
        view.setInputType(inputType);
        view.setSingleLine(true);
        // uiautomator 不导出 hint，只能靠 content-desc 在 dump 里认出该点哪个框。
        view.setContentDescription(caption);
        return view;
    }

    private int dp(float value) {
        return (int) (value * getResources().getDisplayMetrics().density + 0.5f);
    }
}
