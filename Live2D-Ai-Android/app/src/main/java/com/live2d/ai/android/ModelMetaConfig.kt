package com.live2d.ai.android

import android.util.Log
import org.json.JSONObject

/**
 * 从 cdi3.json + pose3.json + model3.json 自动解析的模型元配置。
 *
 * 替代所有硬编码的参数 ID（如 "PARAM_EYE_L_OPEN"）和 Part 名前缀（如 "PartArm"），
 * 使换模型不需要修改 Kotlin 源码。
 *
 * ## 数据来源
 *
 * | 文件 | 用途 |
 * |------|------|
 * | cdi3.json | Parameter Id → GroupId → 语义角色映射 |
 * | pose3.json | Part 组（互斥可见性） |
 * | model3.json | Groups[EyeBlink/LipSync] 显式声明 |
 *
 * ## 语义发现优先级
 *
 * 1. **model3.json Groups** — EyeBlink / LipSync 显式声明（最高优先级）
 * 2. **cdi3.json GroupId** — 按 ParamGroupEyes/ParamGroupMouth 等分组
 * 3. **名称启发式** — 按参数名包含 "Open" / "L" / "R" 等匹配（兜底）
 *
 * ## 向后兼容
 *
 * - 没有 cdi3.json 的旧模型 → `parse()` 返回 `null` → 所有消费者回退到硬编码行为
 * - 未知 GroupId → 仅日志记录，不影响功能
 * - 每个 semantic index 初始化为 -1，消费者调用前检查
 */
class ModelMetaConfig(
    /** 所有参数的元数据列表（匹配 MOC3 参数顺序） */
    val parameters: List<ParameterMeta>,
    /** 语义索引：按功能角色预计算的参数索引 */
    val semantic: SemanticIndex,
    /** Pose 元数据（可选，仅当模型有 pose3.json） */
    val poseMeta: PoseMeta?,
    /** 模型名称（用于调试日志） */
    val modelName: String = ""
) {

    // ============================================================
    //  数据类定义
    // ============================================================

    /**
     * 单个参数的元数据。
     */
    data class ParameterMeta(
        /** 参数 ID（MOC3 中的原始名称），如 "ParamEyeLOpen" */
        val id: String,
        /** 所属参数组 ID，如 "ParamGroupEyes"。无分组时为空字符串 */
        val groupId: String,
        /** 人类可读名称，如 "Eye L_Open" */
        val name: String,
        /** 在 MOC3 参数数组中的 0-based 索引 */
        val index: Int
    )

    /**
     * 按功能角色预计算的参数索引。
     * 所有 -1 表示模型无此参数。
     */
    data class SemanticIndex(
        /** 左眼睁开参数索引 */
        val eyeLeftOpenIdx: Int = -1,
        /** 右眼睁开参数索引 */
        val eyeRightOpenIdx: Int = -1,
        /** 口型同步参数索引列表（可能有多个，如 ParamA + ParamMouthOpenY） */
        val mouthOpenIndices: List<Int> = emptyList(),
        /** 呼吸参数定义列表 */
        val breathParams: List<BreathParamDef> = emptyList(),
        /** 头部旋转 X 参数索引 */
        val angleXIdx: Int = -1,
        /** 头部旋转 Y 参数索引 */
        val angleYIdx: Int = -1,
        /** 头部旋转 Z 参数索引 */
        val angleZIdx: Int = -1,
        /** 身体旋转 X 参数索引（呼吸用） */
        val bodyAngleXIdx: Int = -1
    )

    /**
     * 呼吸参数定义。
     */
    data class BreathParamDef(
        /** 参数索引 */
        val index: Int,
        /** 参数 ID（调试用） */
        val id: String,
        /** 呼吸基线值（正弦波的 DC 偏移） */
        val offset: Float,
        /** 呼吸幅度（正弦波的振幅） */
        val peak: Float,
        /** 呼吸周期（秒） */
        val cycle: Float,
        /** 呼吸权重（0~1），用于混合强度 */
        val weight: Float
    )

    /**
     * Pose 元数据。
     */
    data class PoseMeta(
        /** Pose 分组列表，每组是一组互斥的 part */
        val groups: List<PoseGroup>
    )

    /**
     * Pose 分组。
     */
    data class PoseGroup(
        /** 组内 part ID 列表（如 ["PartArmLA", "PartArmLB"]） */
        val partIds: List<String>,
        /** 驱动此组选择的参数索引（-1 表示无驱动参数，始终选第一个） */
        val driverParamIdx: Int = -1,
        /** 驱动参数的阈值：值 < 阈值 → partIds[0] 可见，否则 → partIds[1] 可见 */
        val driverThreshold: Float = 0.5f,
        /** 组的语义名称（调试用） */
        val groupName: String = ""
    )

    // ============================================================
    //  工厂方法
    // ============================================================

    companion object {
        private const val TAG = "ModelMetaConfig"

        /**
         * 从 cdi3.json、pose3.json 和 model3.json 构建 ModelMetaConfig。
         *
         * @param cdi3Json cdi3.json 的 JSONObject；为 null 时返回 null（旧模型兼容）
         * @param pose3Json pose3.json 的 JSONObject（可选，为 null 时 poseMeta=null）
         * @param model3Json model3.json 的 JSONObject（可选，用于 Groups[EyeBlink/LipSync]）
         * @param paramNameToIndex 参数名→索引映射（从 MOC3 获取）
         * @param partNameToIndex part 名→索引映射（从 MOC3 获取）
         * @return 解析成功返回 ModelMetaConfig，无 cdi3.json 时返回 null
         */
        fun parse(
            cdi3Json: JSONObject?,
            pose3Json: JSONObject?,
            model3Json: JSONObject?,
            paramNameToIndex: Map<String, Int>,
            partNameToIndex: Map<String, Int>
        ): ModelMetaConfig? {
            if (cdi3Json == null) return null

            // ── 解析所有 ParameterMeta ──
            val paramMetas = parseParameterMetas(cdi3Json, paramNameToIndex)
            if (paramMetas.isEmpty()) {
                Log.w(TAG, "No parameters found in cdi3.json — returning null for fallback")
                return null
            }

            // ── 构建语义索引 ──
            val semantic = buildSemanticIndex(paramMetas, model3Json)

            // ── 解析 Pose ──
            val poseMeta = parsePoseMeta(pose3Json, paramMetas)

            Log.i(TAG, "Parsed ${paramMetas.size} params, ${semantic.breathParams.size} breath params" +
                    ", poseGroups=${poseMeta?.groups?.size ?: 0}")

            return ModelMetaConfig(
                parameters = paramMetas,
                semantic = semantic,
                poseMeta = poseMeta
            )
        }

        // ============================================================
        //  参数解析
        // ============================================================

        /**
         * 从 cdi3.json 的 Parameters 数组解析所有参数元数据。
         */
        private fun parseParameterMetas(
            cdi3Json: JSONObject,
            paramNameToIndex: Map<String, Int>
        ): List<ParameterMeta> {
            val paramsArray = cdi3Json.optJSONArray("Parameters") ?: return emptyList()
            val metas = mutableListOf<ParameterMeta>()

            for (i in 0 until paramsArray.length()) {
                val p = paramsArray.getJSONObject(i)
                val id = p.optString("Id", "")
                val groupId = p.optString("GroupId", "")
                val name = p.optString("Name", "")
                // 用 MOC3 的真实索引（而非 cdi3.json 中的顺序索引）
                val index = paramNameToIndex[id] ?: -1
                if (index >= 0) {
                    metas.add(ParameterMeta(id, groupId, name, index))
                } else {
                    Log.v(TAG, "Parameter '$id' not found in MOC3 — skipping")
                }
            }
            return metas
        }

        // ============================================================
        //  语义索引构建
        // ============================================================

        /**
         * 构建 SemanticIndex。
         *
         * 发现优先级：
         * 1. model3.json Groups 中的 EyeBlink / LipSync 显式声明
         * 2. cdi3.json 的 GroupId 分组（ParamGroupEyes / ParamGroupMouth 等）
         * 3. 按参数名启发式匹配
         */
        private fun buildSemanticIndex(
            paramMetas: List<ParameterMeta>,
            model3Json: JSONObject?
        ): SemanticIndex {
            val eyeLeftOpenIdx = resolveEyeParam(paramMetas, model3Json, eyeSide = 'L')
            val eyeRightOpenIdx = resolveEyeParam(paramMetas, model3Json, eyeSide = 'R')
            val mouthOpenIndices = resolveMouthParams(paramMetas, model3Json)
            val breathParams = resolveBreathParams(paramMetas)
            val angleXIdx = paramMetas.find { it.id == "ParamAngleX" }?.index ?: -1
            val angleYIdx = paramMetas.find { it.id == "ParamAngleY" }?.index ?: -1
            val angleZIdx = paramMetas.find { it.id == "ParamAngleZ" }?.index ?: -1
            val bodyAngleXIdx = paramMetas.find { it.id == "ParamBodyAngleX" }?.index ?: -1

            val result = SemanticIndex(
                eyeLeftOpenIdx = eyeLeftOpenIdx,
                eyeRightOpenIdx = eyeRightOpenIdx,
                mouthOpenIndices = mouthOpenIndices,
                breathParams = breathParams,
                angleXIdx = angleXIdx,
                angleYIdx = angleYIdx,
                angleZIdx = angleZIdx,
                bodyAngleXIdx = bodyAngleXIdx
            )

            Log.i(TAG, formatSemanticLog(result, paramMetas))
            return result
        }

        /**
         * 解析眼睛参数索引。
         */
        private fun resolveEyeParam(
            paramMetas: List<ParameterMeta>,
            model3Json: JSONObject?,
            eyeSide: Char
        ): Int {
            // 1. 优先：model3.json Groups[EyeBlink]
            val fromGroups = resolveEyeFromModel3Groups(model3Json, paramMetas, eyeSide)
            if (fromGroups >= 0) return fromGroups

            // 2. 二级：cdi3.json GroupId == ParamGroupEyes
            val fromGroupId = resolveEyeFromGroupId(paramMetas, eyeSide)
            if (fromGroupId >= 0) return fromGroupId

            // 3. 兜底：名称启发式
            return resolveEyeFromName(paramMetas, eyeSide)
        }

        /**
         * 从 model3.json Groups[EyeBlink] 解析眼睛参数。
         */
        private fun resolveEyeFromModel3Groups(
            model3Json: JSONObject?,
            paramMetas: List<ParameterMeta>,
            eyeSide: Char
        ): Int {
            if (model3Json == null) return -1
            val groups = model3Json.optJSONArray("Groups") ?: return -1

            for (i in 0 until groups.length()) {
                val g = groups.getJSONObject(i)
                if (g.optString("Target") != "Parameter") continue
                if (g.optString("Name", "") != "EyeBlink") continue

                val ids = g.optJSONArray("Ids") ?: continue
                for (j in 0 until ids.length()) {
                    val id = ids.getString(j)
                    val meta = paramMetas.find { it.id == id } ?: continue
                    if (eyeSide == 'L' && (id.contains("L", ignoreCase = true) ||
                                id.contains("Left", ignoreCase = true))) {
                        return meta.index
                    }
                    if (eyeSide == 'R' && (id.contains("R", ignoreCase = true) ||
                                id.contains("Right", ignoreCase = true))) {
                        return meta.index
                    }
                }

                // 如果 Group 中有参数但无法区分左右：总长度≥2时取第一个为L、第二个为R
                if (ids.length() >= 2) {
                    val lMeta = paramMetas.find { it.id == ids.getString(0) }
                    val rMeta = paramMetas.find { it.id == ids.getString(1) }
                    if (eyeSide == 'L' && lMeta != null) return lMeta.index
                    if (eyeSide == 'R' && rMeta != null) return rMeta.index
                } else if (ids.length() == 1) {
                    val meta = paramMetas.find { it.id == ids.getString(0) }
                    if (meta != null) return meta.index // 单眼模型：左右共用
                }
            }
            return -1
        }

        /**
         * 从 cdi3.json GroupId == ParamGroupEyes 中按 Open + L/R 匹配。
         */
        private fun resolveEyeFromGroupId(
            paramMetas: List<ParameterMeta>,
            eyeSide: Char
        ): Int {
            val eyeParams = paramMetas.filter { it.groupId == "ParamGroupEyes" }
            if (eyeParams.isEmpty()) return -1

            // 优先匹配包含 "Open" 的参数
            val openParams = eyeParams.filter { it.id.contains("Open", ignoreCase = true) }

            // 在 Open 参数中找左右
            for (ep in openParams) {
                if (eyeSide == 'L' && (ep.id.contains("L", ignoreCase = true) ||
                            ep.id.contains("Left", ignoreCase = true))) {
                    return ep.index
                }
                if (eyeSide == 'R' && (ep.id.contains("R", ignoreCase = true) ||
                            ep.id.contains("Right", ignoreCase = true))) {
                    return ep.index
                }
            }

            // 只有一只眼的模型
            if (openParams.size == 1) return openParams[0].index

            return -1
        }

        /**
         * 用名称启发式匹配眼睛参数（兜底）。
         */
        private fun resolveEyeFromName(
            paramMetas: List<ParameterMeta>,
            eyeSide: Char
        ): Int {
            // 尝试 PARAM_EYE_L_OPEN 或 ParamEyeLOpen 命名约定
            val candidates = listOf(
                "PARAM_EYE_${eyeSide}_OPEN",
                "ParamEye${eyeSide}Open",
                "ParamEye${eyeSide}_Open"
            )
            for (id in candidates) {
                val meta = paramMetas.find { it.id == id }
                if (meta != null) return meta.index
            }
            return -1
        }

        /**
         * 解析口型同步参数索引列表。
         */
        private fun resolveMouthParams(
            paramMetas: List<ParameterMeta>,
            model3Json: JSONObject?
        ): List<Int> {
            val indices = mutableListOf<Int>()

            // 1. 优先：model3.json Groups[LipSync]
            if (model3Json != null) {
                val groups = model3Json.optJSONArray("Groups")
                if (groups != null) {
                    for (i in 0 until groups.length()) {
                        val g = groups.getJSONObject(i)
                        if (g.optString("Target") != "Parameter") continue
                        if (g.optString("Name", "") != "LipSync") continue
                        val ids = g.optJSONArray("Ids") ?: continue
                        for (j in 0 until ids.length()) {
                            val meta = paramMetas.find { it.id == ids.getString(j) }
                            if (meta != null && !indices.contains(meta.index)) {
                                indices.add(meta.index)
                            }
                        }
                    }
                }
            }

            // 2. 二级：cdi3.json ParamGroupMouth + 已知口型参数名
            if (indices.isEmpty()) {
                val mouthParams = paramMetas.filter { it.groupId == "ParamGroupMouth" }
                // 按优先级尝试 ParamA, ParamMouthOpenY
                for (preferred in listOf("ParamA", "ParamMouthOpenY", "ParamMouthOpen")) {
                    val meta = mouthParams.find { it.id == preferred }
                    if (meta != null && !indices.contains(meta.index)) {
                        indices.add(meta.index)
                    }
                }
                // 兜底：任何包含 "Mouth"+"Open" 的参数
                if (indices.isEmpty()) {
                    for (mp in mouthParams) {
                        if (mp.id.contains("Mouth", ignoreCase = true) &&
                            mp.id.contains("Open", ignoreCase = true) &&
                            !indices.contains(mp.index)) {
                            indices.add(mp.index)
                        }
                    }
                }
            }

            return indices
        }

        /**
         * 解析呼吸参数定义。
         *
         * 从所有参数中匹配已知的呼吸参数 ID，使用 SDK 参考实现的默认值。
         */
        private fun resolveBreathParams(paramMetas: List<ParameterMeta>): List<BreathParamDef> {
            // spoq-fix：呼吸只驱动胸部（ParamBreath）——官方默认把 ParamAngleX/Y/Z（头部旋转）
            // 也归入呼吸（±15°），在 bai 等模型上视觉为“头部正弦摆动”，用户实锤异常。
            // 头部姿态由 motion/表情控制，呼吸语义 = 胸部起伏。
            val knownBreathIds = setOf(
                "ParamBreath"
            )

            val breathDefaults = mapOf(
                "ParamBreath" to BreathParamDef(0, "ParamBreath", 0.5f, 0.5f, 3.2345f, 0.5f)
            )

            val defs = mutableListOf<BreathParamDef>()
            for (pm in paramMetas) {
                if (pm.id in knownBreathIds) {
                    breathDefaults[pm.id]?.let { defaultDef ->
                        defs.add(defaultDef.copy(index = pm.index))
                    }
                }
            }
            return defs
        }

        /**
         * 格式化语义索引调试日志。
         */
        private fun formatSemanticLog(si: SemanticIndex, paramMetas: List<ParameterMeta>): String {
            return "SemanticIndex: " +
                "eyeLeftOpen=${findParamId(si.eyeLeftOpenIdx, paramMetas)}(idx=${si.eyeLeftOpenIdx}), " +
                "eyeRightOpen=${findParamId(si.eyeRightOpenIdx, paramMetas)}(idx=${si.eyeRightOpenIdx}), " +
                "mouth=$si.mouthOpenIndices, " +
                "breath=${si.breathParams.size} params, " +
                "angleX=$si.angleXIdx, angleY=$si.angleYIdx, " +
                "angleZ=$si.angleZIdx, bodyAngleX=$si.bodyAngleXIdx"
        }

        // ============================================================
        //  Pose 解析
        // ============================================================

        /**
         * 解析 pose3.json → PoseMeta。
         */
        private fun parsePoseMeta(
            pose3Json: JSONObject?,
            paramMetas: List<ParameterMeta>
        ): PoseMeta? {
            if (pose3Json == null) return null
            try {
                val groupsArray = pose3Json.optJSONArray("Groups") ?: return null
                val groups = mutableListOf<PoseGroup>()

                for (i in 0 until groupsArray.length()) {
                    val group = groupsArray.getJSONArray(i)
                    val partIds = mutableListOf<String>()

                    for (j in 0 until group.length()) {
                        val part = group.getJSONObject(j)
                        val id = part.getString("Id")
                        partIds.add(id)
                    }

                    if (partIds.isNotEmpty()) {
                        val driverParamIdx = findDriverParam(partIds, paramMetas)
                        val groupName = deriveGroupName(partIds)
                        groups.add(PoseGroup(
                            partIds = partIds,
                            driverParamIdx = driverParamIdx,
                            groupName = groupName
                        ))
                    }
                }

                if (groups.isEmpty()) return null

                val meta = PoseMeta(groups = groups)
                val partCount = groups.sumOf { it.partIds.size }
                Log.i(TAG, "PoseMeta: ${groups.size} groups ($partCount parts)")
                return meta
            } catch (e: Exception) {
                Log.w(TAG, "Failed to parse pose3.json: ${e.message}")
                return null
            }
        }

        /**
         * 从 part ID 推导驱动参数索引。
         *
         * 规则：
         * - PartArmLA → 参数组 ParamGroupArmLA
         * - 在参数组中找 Id 包含 "01" 或 "Angle" 的参数作为驱动参数
         * - 如未找到特定参数，使用组内第一个可用的参数
         */
        private fun findDriverParam(
            partIds: List<String>,
            paramMetas: List<ParameterMeta>
        ): Int {
            if (partIds.isEmpty()) return -1
            val paramGroupId = deriveParamGroupId(partIds[0]) ?: return -1

            val groupParams = paramMetas.filter { it.groupId == paramGroupId }
            if (groupParams.isEmpty()) return -1

            // 优先找 Id 包含 "01" 或 "Angle" 的参数（肩部旋转角度）
            val angleParam = groupParams.find {
                it.id.contains("01", ignoreCase = true) ||
                    it.id.contains("Angle", ignoreCase = true)
            }
            return angleParam?.index ?: groupParams.first().index
        }

        /**
         * 从 part ID 推导对应的参数组 ID。
         * PartArmLA → ParamGroupArmLA
         */
        private fun deriveParamGroupId(partId: String): String? {
            if (!partId.startsWith("Part")) return null
            val suffix = partId.removePrefix("Part")
            return "ParamGroup$suffix"
        }

        /**
         * 从 part ID 列表推导人类可读的组名。
         */
        private fun deriveGroupName(partIds: List<String>): String {
            if (partIds.isEmpty()) return ""
            val id = partIds[0]
            if (id.startsWith("Part")) {
                val suffix = id.removePrefix("Part")
                // CamelCase → 空格分隔
                return suffix.replace(Regex("([a-z])([A-Z])"), "$1 $2")
                    .replace(Regex("([A-Z])([A-Z][a-z])"), "$1 $2")
            }
            return id
        }

        /**
         * 通过参数索引查找参数 ID（用于调试日志）。
         */
        private fun findParamId(index: Int, paramMetas: List<ParameterMeta>): String {
            return paramMetas.find { it.index == index }?.id ?: "?"
        }
    }
}
