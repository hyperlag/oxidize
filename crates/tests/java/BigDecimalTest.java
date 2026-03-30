import java.math.BigDecimal;
import java.math.RoundingMode;

public class BigDecimalTest {
    public static void main(String[] args) {
        // Construction
        BigDecimal a = new BigDecimal("3.14159");
        BigDecimal b = new BigDecimal("2.71828");
        System.out.println("a = " + a);
        System.out.println("b = " + b);

        // Arithmetic
        System.out.println("a + b = " + a.add(b));
        System.out.println("a - b = " + a.subtract(b));
        System.out.println("a * b = " + a.multiply(b));

        // Division with scale and rounding
        BigDecimal ten = new BigDecimal("10");
        BigDecimal three = new BigDecimal("3");
        BigDecimal div = ten.divide(three, 4, RoundingMode.HALF_UP);
        System.out.println("10 / 3 (scale=4, HALF_UP) = " + div);

        // Comparison
        BigDecimal x = new BigDecimal("2.0");
        BigDecimal y = new BigDecimal("2.00");
        System.out.println("2.0 compareTo 2.00 = " + x.compareTo(y));
        System.out.println("2.0 equals 2.00 = " + x.equals(y));

        // Constants
        System.out.println("ZERO = " + BigDecimal.ZERO);
        System.out.println("ONE = " + BigDecimal.ONE);
        System.out.println("TEN = " + BigDecimal.TEN);

        // Scale operations
        BigDecimal pi = new BigDecimal("3.14159265");
        BigDecimal rounded = pi.setScale(2, RoundingMode.HALF_UP);
        System.out.println("pi rounded to 2 = " + rounded);

        // Abs and negate
        BigDecimal neg = new BigDecimal("-5.5");
        System.out.println("abs(-5.5) = " + neg.abs());
        System.out.println("negate(-5.5) = " + neg.negate());

        // valueOf
        BigDecimal fromLong = BigDecimal.valueOf(42);
        System.out.println("valueOf(42) = " + fromLong);

        // Conversion
        BigDecimal conv = new BigDecimal("123.456");
        System.out.println("intValue = " + conv.intValue());
        System.out.println("longValue = " + conv.longValue());
        System.out.println("doubleValue = " + conv.doubleValue());

        // Signum
        System.out.println("signum(3.14) = " + a.signum());
        System.out.println("signum(-5.5) = " + neg.signum());
        System.out.println("signum(0) = " + BigDecimal.ZERO.signum());

        // Pow
        BigDecimal two = new BigDecimal("2");
        System.out.println("2^10 = " + two.pow(10));

        // Max / Min
        System.out.println("max(a, b) = " + a.max(b));
        System.out.println("min(a, b) = " + a.min(b));
    }
}
