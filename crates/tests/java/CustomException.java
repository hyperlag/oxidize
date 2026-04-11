class AppException extends RuntimeException {
    public AppException(String message) {
        super(message);
    }
}

class DetailedException extends AppException {
    public DetailedException(String message) {
        super(message);
    }
}

class CustomException {
    static void validate(int x) {
        if (x < 0) {
            throw new AppException("negative value: " + x);
        }
    }

    static void checkDetail(String s) {
        if (s.isEmpty()) {
            throw new DetailedException("empty string");
        }
    }

    public static void main(String[] args) {
        try {
            validate(5);
            System.out.println("ok: 5");
        } catch (AppException e) {
            System.out.println("error: " + e.getMessage());
        }

        try {
            validate(-3);
        } catch (AppException e) {
            System.out.println("caught: " + e.getMessage());
        }

        try {
            checkDetail("hello");
            System.out.println("detail ok");
        } catch (AppException e) {
            System.out.println("detail error: " + e.getMessage());
        }

        try {
            checkDetail("");
        } catch (AppException e) {
            System.out.println("detail caught: " + e.getMessage());
        }
    }
}
